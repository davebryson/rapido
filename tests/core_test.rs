use abci::*;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    BinaryValue, ObjectHash, Snapshot, TemporaryDB,
};
use std::sync::Arc;
use std::{borrow::Cow, convert::AsRef};

use rapido::{AppBuilder, AppModule, Context, SignedTransaction};

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Default)]
pub struct Count(u16);
impl Count {
    pub fn inc(&mut self) {
        self.0 += 1;
    }
}
impl BinaryValue for Count {
    fn to_bytes(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, anyhow::Error> {
        Count::try_from_slice(bytes.as_ref()).map_err(From::from)
    }
}
impl ObjectHash for Count {
    fn object_hash(&self) -> Hash {
        exonum_crypto::hash(&BinaryValue::to_bytes(self))
    }
}

#[derive(Debug)]
pub(crate) struct CountStore<T: Access> {
    access: T,
}
impl<T: Access> CountStore<T> {
    pub fn new(access: T) -> Self {
        Self { access }
    }

    pub fn get_count(&self) -> Count {
        self.access
            .get_proof_entry("counter")
            .get()
            .unwrap_or_default()
    }
}
impl<T: Access> CountStore<T>
where
    T::Base: RawAccessMut,
{
    pub fn increment(&mut self) {
        let mut count = self.get_count();
        count.inc();
        self.access.get_proof_entry("counter").set(count);
    }
}

pub struct CounterExample;
impl AppModule for CounterExample {
    fn name(&self) -> &'static str {
        "counter"
    }

    fn handle_tx(&self, ctx: Context) -> Result<(), anyhow::Error> {
        let mut store = CountStore::new(ctx.fork);
        store.increment();
        Ok(())
    }

    fn handle_query(
        &self,
        _path: &str,
        _key: Vec<u8>,
        snapshot: &Box<dyn Snapshot>,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let store = CountStore::new(snapshot);
        let cnt = store.get_count();
        Ok(cnt.to_bytes())
    }
}

// Helpers
fn generate_tx() -> SignedTransaction {
    SignedTransaction::new([0u8; 10].to_vec(), "counter", 0u8, vec![0])
}

fn send_tx() -> RequestDeliverTx {
    let tx = generate_tx().encode();
    let mut req = RequestDeliverTx::new();
    req.set_tx(tx.clone());
    req
}

#[test]
fn test_basics() {
    let db = Arc::new(TemporaryDB::new());
    let mut node = AppBuilder::new(db)
        .add_service(Box::new(CounterExample {}))
        .finish();

    node.init_chain(&RequestInitChain::new());
    let resp = node.commit(&RequestCommit::new());
    assert!(resp.data.len() > 0);

    let resp = node.deliver_tx(&send_tx());
    assert_eq!(0u32, resp.code);
    let c1 = node.commit(&RequestCommit::new());
    assert_ne!(resp.data, c1.data);

    let mut query = RequestQuery::new();
    query.path = "counter/".into(); // <= this is tricky how to improve?!  need slash at end
    let resp = node.query(&query);
    assert_eq!(0u32, resp.code);
    let c = Count::try_from_slice(&resp.value[..]).unwrap();
    assert_eq!(1, c.0);
}
