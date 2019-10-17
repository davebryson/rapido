use abci::*;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{gen_keypair, Hash, SecretKey};
use exonum_merkledb::{
    Fork, ObjectAccess, ObjectHash, ProofMapIndex, RefMut, Snapshot, TemporaryDB,
};
use std::sync::Arc;

use super::{
    sign_transaction, AppBuilder, Node, QueryResult, Service, SignedTransaction, Transaction,
    TxResult,
};

/// Do a full test through abci framework:
/// Check Tx
/// Deliver Tx
/// Query
/// Example:  Counter: Increment count object for user
const ROUTE_NAME: &str = "counter_test";

// Storage for the app
pub struct SchemaStore<T: ObjectAccess>(T);
impl<T: ObjectAccess> SchemaStore<T> {
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    pub fn add_account(&self, account: &Vec<u8>) {
        self.store().put(&account, 0u8);
    }

    pub fn store(&self) -> RefMut<ProofMapIndex<T, Vec<u8>, u8>> {
        self.0.get_object(ROUTE_NAME)
    }

    pub fn get_current_count(&self, account: &Vec<u8>) -> u8 {
        match self.store().get(account) {
            Some(v) => v,
            None => 0u8,
        }
    }

    pub fn set_next_count(&self, account: &Vec<u8>, value: u8) {
        self.store().put(account, value);
    }
}

// Application Msg
#[derive(BorshSerialize, BorshDeserialize, Debug, Default)]
struct SetCountMsg(u8);
impl Transaction for SetCountMsg {
    // App logic:
    // Set the value in state. Rule: the 'value' must be the expected next number.
    // If not, error.
    fn execute(&self, sender: Vec<u8>, fork: &Fork) -> TxResult {
        let schema = SchemaStore::new(fork);
        let current = schema.get_current_count(&sender);
        let expected_next_value = current + 1;
        if self.0 != expected_next_value {
            return TxResult::error(1, format!("msg value <= current state"));
        }
        schema.set_next_count(&sender, expected_next_value);
        TxResult::ok()
    }
}

// Service
struct CounterService;
impl Service for CounterService {
    fn route(&self) -> &'static str {
        ROUTE_NAME
    }

    fn genesis(&self, fork: &Fork) -> TxResult {
        let dave = vec![1u8; 32]; // Genesis account
        let schema = SchemaStore::new(fork);
        schema.add_account(&dave);
        TxResult::ok()
    }

    fn decode_tx(
        &self,
        _txid: u8,
        payload: Vec<u8>,
    ) -> Result<Box<dyn Transaction>, std::io::Error> {
        // We don't check for txid
        let m = SetCountMsg::try_from_slice(&payload[..])?;
        Ok(Box::new(m))
    }

    fn query(&self, path: &str, key: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> QueryResult {
        if path == "/" {
            let schema = SchemaStore::new(snapshot);
            let acct = key.clone();
            //if acct.is_err() {
            //    QueryResult::error(10);
            // }
            let value = schema.get_current_count(&acct);
            return QueryResult::ok(vec![value]);
        }

        if path == "/one" {
            return QueryResult::ok(vec![0x1]);
        }

        if path == "/two" {
            return QueryResult::ok(vec![0x2]);
        }

        QueryResult::error(10)
    }

    fn root_hash(&self, fork: &Fork) -> Hash {
        let schema = SchemaStore::new(fork);
        schema.store().object_hash()
    }
}

// CheckTx Handler
fn my_validate_tx(tx: &SignedTransaction, _snapshot: &Box<dyn Snapshot>) -> TxResult {
    if tx.sender != vec![1u8; 32] {
        return TxResult::error(1, "bad account amigo..");
    }
    TxResult::ok()
}

// Test helpers
fn gen_and_sign_tx(acct: Vec<u8>, sk: &SecretKey, msg: SetCountMsg) -> Vec<u8> {
    let mut signed = SignedTransaction::new(acct, ROUTE_NAME, 0, msg);
    sign_transaction(&mut signed, &sk);
    let encoded = signed.try_to_vec();
    assert!(encoded.is_ok());
    encoded.unwrap()
}

fn assert_check_tx(app: &mut Node, bits: Vec<u8>, expected_code: u32) {
    let mut req = RequestCheckTx::new();
    req.set_tx(bits);
    let resp = app.check_tx(&req);
    assert_eq!(expected_code, resp.code);
}

fn assert_deliver_tx(app: &mut Node, bits: Vec<u8>, expected_code: u32) -> Vec<u8> {
    let mut req = RequestDeliverTx::new();
    req.set_tx(bits);
    let resp = app.deliver_tx(&req);
    assert_eq!(expected_code, resp.code);
    // return the latest app hash
    app.commit(&RequestCommit::new()).data
}

#[test]
fn test_abci_works() {
    let db = Arc::new(TemporaryDB::new());
    let mut app = AppBuilder::new(db.clone())
        .set_validation_handler(my_validate_tx)
        .add_service(Box::new(CounterService {}))
        .finish();

    let (_pk, sk) = gen_keypair();
    let dave = vec![1u8; 32]; // test account add on genesis() in service

    assert_check_tx(
        &mut app,
        gen_and_sign_tx(dave.clone(), &sk, SetCountMsg(1)),
        0u32,
    );
    // Should fail (bad account)
    assert_check_tx(
        &mut app,
        gen_and_sign_tx(vec![3u8; 32], &sk, SetCountMsg(1)),
        1u32,
    );

    let root1 = assert_deliver_tx(
        &mut app,
        gen_and_sign_tx(dave.clone(), &sk, SetCountMsg(1)),
        0u32,
    );
    let root2 = assert_deliver_tx(
        &mut app,
        gen_and_sign_tx(dave.clone(), &sk, SetCountMsg(1)),
        1u32,
    ); // fail
    assert_eq!(root1, root2); // shouldn't change

    let root3 = assert_deliver_tx(
        &mut app,
        gen_and_sign_tx(dave.clone(), &sk, SetCountMsg(2)),
        0u32,
    );
    assert_ne!(root1, root3);

    // Check queries
    {
        let mut query = RequestQuery::new();
        query.path = format!("{}/", ROUTE_NAME);
        query.data = dave.clone();
        let qresp = app.query(&query);
        assert_eq!(0u32, qresp.code);
        assert_eq!(vec![2], qresp.value);
    }

    {
        // Should fail
        let mut query = RequestQuery::new();
        query.path = "shouldfail".into();
        query.data = dave.to_vec();
        let qresp = app.query(&query);
        assert_eq!(103u32, qresp.code);
    }

    {
        // Should fail
        let mut query = RequestQuery::new();
        query.path = "/".into();
        query.data = dave.to_vec();
        let qresp = app.query(&query);
        assert_eq!(103u32, qresp.code);
    }

    {
        // Should fail
        let mut query = RequestQuery::new();
        query.path = "noserviceregistered/".into();
        query.data = dave.to_vec();
        let qresp = app.query(&query);
        assert_eq!(100u32, qresp.code);
    }

    {
        let mut query = RequestQuery::new();
        query.path = format!("{}/one", ROUTE_NAME);
        query.data = vec![];
        let qresp = app.query(&query);
        assert_eq!(0u32, qresp.code);
        assert_eq!(vec![0x1], qresp.value);
    }

    {
        let mut query = RequestQuery::new();
        query.path = format!("{}/two", ROUTE_NAME);
        query.data = vec![];
        let qresp = app.query(&query);
        assert_eq!(0u32, qresp.code);
        assert_eq!(vec![0x2], qresp.value);
    }
}
