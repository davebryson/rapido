use abci::*;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{Fork, Snapshot, TemporaryDB};
use rapido::{AppBuilder, QueryResult, Service, Transaction, TxResult};
use std::panic;
use std::sync::Arc;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
struct TMsg {
    pub val: u8,
}

pub struct S1;
impl Service for S1 {
    fn route(&self) -> String {
        "s1".into()
    }

    fn execute(&self, _tx: &Transaction, _fork: &Fork) -> TxResult {
        TxResult::ok()
    }

    fn query(&self, _path: String, _key: Vec<u8>, _snapshot: &Box<dyn Snapshot>) -> QueryResult {
        QueryResult::ok(vec![])
    }

    fn root_hash(&self, _fork: &Fork) -> Hash {
        Hash::zero()
    }
}

pub struct S2;
impl Service for S2 {
    fn route(&self) -> String {
        "s2".into()
    }

    fn execute(&self, _tx: &Transaction, _fork: &Fork) -> TxResult {
        TxResult::error(11u32, "")
    }

    fn query(&self, _path: String, _key: Vec<u8>, _snapshot: &Box<dyn Snapshot>) -> QueryResult {
        QueryResult::ok(vec![])
    }

    fn root_hash(&self, _fork: &Fork) -> Hash {
        Hash::zero()
    }
}

#[test]
fn test_builder() {
    let db = Arc::new(TemporaryDB::new());
    let mut app = AppBuilder::new(db)
        .add_service(Box::new(S1 {}))
        .add_service(Box::new(S2 {}))
        .finish();
    {
        let tx1 = Transaction::new("s2", 0, TMsg { val: 1 })
            .try_to_vec()
            .unwrap();
        let mut r1 = RequestDeliverTx::new();
        r1.tx = tx1;
        let result = app.deliver_tx(&r1);
        assert_eq!(11u32, result.code);
    }

    {
        let tx1 = Transaction::new("s1", 0, TMsg { val: 1 })
            .try_to_vec()
            .unwrap();
        let mut r1 = RequestDeliverTx::new();
        r1.tx = tx1;
        let result = app.deliver_tx(&r1);
        assert_eq!(0u32, result.code);
    }
}

#[test]
fn test_missing_service() {
    let db = Arc::new(TemporaryDB::new());
    let result = panic::catch_unwind(|| {
        AppBuilder::new(db).finish();
    });
    assert!(result.is_err());
}
