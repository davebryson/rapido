use abci::*;
use exonum_merkledb::TemporaryDB;
use rapido::{AppBuilder, IntoProtoBytes, QueryResult, Service, Tx, TxResult};
use std::sync::Arc;

use exonum_crypto::Hash;
use exonum_merkledb::{Database, Fork, ObjectAccess, ObjectHash, ProofMapIndex, RefMut, Snapshot};

const ROUTE_NAME: &str = "exapp";

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
pub struct TestingService;

impl Service for TestingService {
    fn route(&self) -> String {
        ROUTE_NAME.into()
    }

    fn execute(&self, tx: &Tx, fork: &Fork) -> TxResult {
        let schema = SchemaStore::new(fork);
        let mut store = schema.state();

        // Make the tx fail
        if tx.msg == vec![251] {
            return TxResult::error(1, "");
        }
        // Updating 2 different accounts at once
        store.put(&1u32, tx.msg.clone());
        store.put(&2u32, tx.msg.clone());
        TxResult::ok()
    }

    fn query(&self, _path: String, _key: Vec<u8>, _snapshot: &Box<dyn Snapshot>) -> QueryResult {
        QueryResult::ok(vec![])
    }

    fn root_hash(&self, fork: &Fork) -> Hash {
        let schema = SchemaStore::new(fork);
        schema.state().object_hash()
    }
}

fn make_tx(value: u8) -> RequestDeliverTx {
    let mut tx = Tx::new();
    tx.route = ROUTE_NAME.into();
    tx.msg = vec![value];
    let bits = tx.into_proto_bytes().unwrap();
    let mut req = RequestDeliverTx::new();
    req.set_tx(bits);
    req
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_txs_and_query() {
        let db = Arc::new(TemporaryDB::new());
        let db2 = db.clone();
        let mut app = AppBuilder::new(db)
            .add_service(Box::new(TestingService {}))
            .finish();

        let expected_root_1 = vec![
            103, 180, 121, 63, 106, 92, 133, 154, 190, 221, 126, 84, 47, 183, 75, 18, 109, 30, 144,
            212, 87, 58, 242, 68, 0, 95, 13, 76, 78, 204, 101, 180,
        ];

        let expected_root_2 = vec![
            159, 89, 166, 103, 1, 27, 93, 210, 83, 231, 103, 194, 43, 97, 164, 207, 13, 235, 165,
            145, 187, 114, 118, 202, 213, 41, 72, 115, 176, 219, 224, 255,
        ];

        {
            // create some Txs
            for i in 0..250 {
                let req = make_tx(i);
                let resp = app.deliver_tx(&req);
                assert_eq!(0u32, resp.code);
            }
        }

        {
            // commit
            let commit = app.commit(&RequestCommit::new());
            assert_eq!(expected_root_1, commit.data);
        }

        {
            // send failed tx.  Should not commit
            let req = make_tx(251); // 251 tells the service to return code 1
            let resp = app.deliver_tx(&req);
            assert_eq!(1u32, resp.code);
            let commit = app.commit(&RequestCommit::new());
            assert_eq!(expected_root_1, commit.data);
        }

        {
            // check snapshot
            let snapshot = db2.snapshot();
            let store = SchemaStore::new(&snapshot).state();

            let result1 = store.get(&1u32);
            assert!(result1.is_some());
            assert_eq!(result1.unwrap(), vec![249]);

            let result2 = store.get(&2u32);
            assert!(result2.is_some());
            assert_eq!(result2.unwrap(), vec![249]);
        }

        {
            // send another Tx - root changes
            let req = make_tx(252);
            let resp = app.deliver_tx(&req);
            assert_eq!(0u32, resp.code);
            let commit = app.commit(&RequestCommit::new());
            assert_eq!(expected_root_2, commit.data);

            let snapshot = db2.snapshot();
            let store = SchemaStore::new(&snapshot).state();
            let result2 = store.get(&2u32);
            assert!(result2.is_some());
            assert_eq!(result2.unwrap(), vec![252]);
        }
    }
}
