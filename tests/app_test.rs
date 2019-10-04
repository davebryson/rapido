use abci::*;
use exonum_merkledb::TemporaryDB;
use rapido::{AppBuilder, IntoProtoBytes, Node, Service, Tx, TxResult};
use std::sync::Arc;

use exonum_crypto::Hash;
use exonum_merkledb::{Database, Fork, ObjectAccess, ObjectHash, ProofMapIndex, RefMut};

/// Example app cryptocurrency
/// Schema
/// Service
/// Msgs

const ROUTE_NAME: &str = "sampleapp";

// Storage
pub struct SchemaStore<T: ObjectAccess>(T);

impl<T: ObjectAccess> SchemaStore<T> {
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    pub fn state(&self) -> RefMut<ProofMapIndex<T, u32, Vec<u8>>> {
        self.0.get_object(ROUTE_NAME)
    }
}

// Service
pub struct SimpleHandler;

impl Service for SimpleHandler {
    fn route(&self) -> String {
        ROUTE_NAME.into()
    }

    fn execute(&self, tx: &Tx, fork: &Fork) -> TxResult {
        let schema = SchemaStore::new(fork);
        let mut store = schema.state();
        store.put(&1u32, tx.msg.clone());
        TxResult::ok()
    }

    fn root_hash(&self, fork: &Fork) -> Hash {
        let schema = SchemaStore::new(fork);
        schema.state().object_hash()
    }
}

fn send_and_check_code(app: &mut Node, val: Vec<u8>) {
    let mut tx = Tx::new();
    tx.route = ROUTE_NAME.into();
    tx.msg = val;
    let bits = tx.into_proto_bytes().unwrap();

    let mut req = RequestDeliverTx::new();
    req.set_tx(bits);
    let resp = app.deliver_tx(&req);
    assert_eq!(0u32, resp.code);
}

fn commit_and_check_storage(app: &mut Node, db: Arc<TemporaryDB>, expected: Vec<u8>) {
    app.commit(&RequestCommit::new());
    let snapshot = db.snapshot();
    let schema = SchemaStore::new(&snapshot);
    let store = schema.state();
    assert_eq!(store.get(&1u32).unwrap(), expected);
}

#[test]
fn test_app_basics() {
    // Create the app
    let db = Arc::new(TemporaryDB::new());
    // used for testing
    let localaccess = db.clone();
    let mut app = AppBuilder::new(db)
        .add_service(Box::new(SimpleHandler {}))
        .finish();

    send_and_check_code(&mut app, vec![0x1]);
    send_and_check_code(&mut app, vec![0x2]);
    send_and_check_code(&mut app, vec![0x3]);

    commit_and_check_storage(&mut app, localaccess.clone(), vec![0x3]);

    send_and_check_code(&mut app, vec![0x4]);
    send_and_check_code(&mut app, vec![0x5]);

    commit_and_check_storage(&mut app, localaccess.clone(), vec![0x5]);
}
