use anyhow::bail;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    BinaryValue, ObjectHash, Snapshot, TemporaryDB,
};
use rapido::*;
use std::sync::Arc;
use std::{borrow::Cow, convert::AsRef};

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

    fn handle_tx(&self, ctx: &Context) -> Result<(), anyhow::Error> {
        let mut store = CountStore::new(ctx.fork);
        store.increment();
        // Emit an event from this call
        ctx.dispatch_event("count", &[("added", "one")]);
        Ok(())
    }

    fn handle_query(
        &self,
        path: &str,
        _key: Vec<u8>,
        snapshot: &Box<dyn Snapshot>,
    ) -> Result<Vec<u8>, anyhow::Error> {
        match path {
            "/" => query_count(snapshot),
            "/random" => query_random(),
            _ => bail!(""),
        }
    }
}

// Queries
fn query_count(snapshot: &Box<dyn Snapshot>) -> Result<Vec<u8>, anyhow::Error> {
    let store = CountStore::new(snapshot);
    let cnt = store.get_count();
    Ok(format!("count is: {}", cnt.0).as_bytes().to_vec())
}

fn query_random() -> Result<Vec<u8>, anyhow::Error> {
    Ok("hello there".as_bytes().to_vec())
}

fn main() {
    let db = Arc::new(TemporaryDB::new());
    let node = AppBuilder::new(db)
        //.add_service(Box::new(CounterExample {}))
        .register_apps(vec![Box::new(CounterExample {})])
        .finish();

    abci::run_local(node);
}
