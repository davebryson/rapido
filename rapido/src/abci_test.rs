use abci::*;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{gen_keypair, Hash, SecretKey};
use exonum_merkledb::{
    Database, Fork, ObjectAccess, ObjectHash, ProofMapIndex, RefMut, Snapshot, TemporaryDB,
};
use std::sync::Arc;

use super::{
    sign_transaction, AccountId, AppBuilder, Node, QueryResult, Service, SignedTransaction,
    Transaction, TxResult,
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

    pub fn add_account(&self, account: &AccountId) {
        self.store().put(&account, 0u8);
    }

    pub fn store(&self) -> RefMut<ProofMapIndex<T, AccountId, u8>> {
        self.0.get_object(ROUTE_NAME)
    }

    pub fn get_current_count(&self, account: &AccountId) -> u8 {
        match self.store().get(account) {
            Some(v) => v,
            None => 0u8,
        }
    }

    pub fn set_next_count(&self, account: &AccountId, value: u8) {
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
    fn execute(&self, sender: AccountId, fork: &Fork) -> TxResult {
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
    fn route(&self) -> String {
        ROUTE_NAME.into()
    }

    fn decode_tx(
        &self,
        _msgid: u16,
        payload: Vec<u8>,
    ) -> Result<Box<dyn Transaction>, std::io::Error> {
        // We don't check for msgid
        let m = SetCountMsg::try_from_slice(&payload[..])?;
        Ok(Box::new(m))
    }

    fn query(&self, _path: String, key: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> QueryResult {
        let schema = SchemaStore::new(snapshot);
        let mut acct = [0u8; 32];
        if key.len() != 32 {
            QueryResult::error(10);
        }
        acct.copy_from_slice(&key[..]);
        let value = schema.get_current_count(&acct);
        QueryResult::ok(vec![value])
    }

    fn root_hash(&self, fork: &Fork) -> Hash {
        let schema = SchemaStore::new(fork);
        schema.store().object_hash()
    }
}

// CheckTx Handler
fn my_validate_tx(tx: &SignedTransaction, _snapshot: &Box<dyn Snapshot>) -> TxResult {
    if tx.sender != [1u8; 32] {
        return TxResult::error(1, "bad account amigo..");
    }
    TxResult::ok()
}

// Test helpers
fn gen_and_sign_tx(acct: AccountId, sk: &SecretKey, msg: SetCountMsg) -> Vec<u8> {
    let mut signed = SignedTransaction::new(acct, ROUTE_NAME, 0u16, msg);
    assert!(sign_transaction(&mut signed, &sk).is_ok());
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
    let dave = [1u8; 32]; // test account
                          // Genesis
    let f = db.fork();
    let schema = SchemaStore::new(&f);
    schema.add_account(&dave);
    assert!(db.merge(f.into_patch()).is_ok());

    // Should pass
    assert_check_tx(&mut app, gen_and_sign_tx(dave, &sk, SetCountMsg(1)), 0u32);
    // Should fail (bad account)
    assert_check_tx(
        &mut app,
        gen_and_sign_tx([3u8; 32], &sk, SetCountMsg(1)),
        1u32,
    );

    let root1 = assert_deliver_tx(&mut app, gen_and_sign_tx(dave, &sk, SetCountMsg(1)), 0u32);
    let root2 = assert_deliver_tx(&mut app, gen_and_sign_tx(dave, &sk, SetCountMsg(1)), 1u32); // fail
    assert_eq!(root1, root2); // shouldn't change

    let root3 = assert_deliver_tx(&mut app, gen_and_sign_tx(dave, &sk, SetCountMsg(2)), 0u32);
    assert_ne!(root1, root3);

    // Check state via a query.
    let mut query = RequestQuery::new();
    query.path = format!("{}**whatever", ROUTE_NAME);
    query.data = base64::encode(&dave[..]).as_bytes().to_vec();
    let qresp = app.query(&query);
    assert_eq!(0u32, qresp.code);

    let v = base64::decode(&qresp.value[..]);
    assert!(v.is_ok());
    assert_eq!(vec![2], v.unwrap());
}