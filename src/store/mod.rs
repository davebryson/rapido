pub use self::schema::{AppState, Schema, StoreKey, StoreValue};

mod schema;

use exonum_merkledb::{BinaryValue, Database, Fork, ObjectHash};
use std::collections::HashMap;
use std::sync::Arc;

enum WriteOp {
    Put(StoreValue, bool),
    Delete,
}

/// Write batch is used to collect changes to state across the 'consensus' connection.
/// DeliverTx is called multiple times before commit, so we must 'hold' all the changes
/// in a batch. Flow:
///
/// ```text   
/// begin_block()
///   ... deliverTx() for each tx to process
/// end_block()
/// commit()
/// ```
/// Athough it may live across all 3 threads, only 1 will be writing to it.  This could
/// change in the future.
pub struct StateStore {
    // Not wrapped in RWLock as only one thread will write to it
    cache: HashMap<StoreValue, WriteOp>,
    db: Arc<dyn Database>,
}

impl StateStore {
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self {
            cache: HashMap::new(),
            db,
        }
    }

    pub fn put(&mut self, k: Vec<u8>, v: Vec<u8>) {
        self.save(k, WriteOp::Put(v, true));
    }

    pub fn get(&mut self, key: Vec<u8>) -> Option<Vec<u8>> {
        // 1. Try the cache
        if let Some(op) = self.cache.get(&key) {
            return match op {
                WriteOp::Put(v, _) => Some(v.clone()),
                _ => None,
            };
        }

        // 2. Try db. If it's there add to cache
        match self.get_account(&key) {
            Some(v) => {
                self.save(key, WriteOp::Put(v.clone(), false));
                Some(v)
            }
            _ => None,
        }
    }

    pub fn remove(&mut self, k: Vec<u8>) {
        self.save(k, WriteOp::Delete);
    }

    /// Called from ABCI commit() to merge all changes to db
    pub fn commit(&mut self, fork: &Fork) -> Vec<u8> {
        let schema = Schema::new(fork);
        let mut accounts = schema.state();
        for (k, op) in self.cache.drain() {
            match op {
                WriteOp::Put(v, true) => accounts.put(&k, v.clone()), /* only dirty writes */
                WriteOp::Delete => accounts.remove(&k),
                _ => unimplemented!(),
            }
        }
        let hash = accounts.object_hash();
        hash.to_bytes()
    }

    pub fn reset_cache(&mut self) {
        self.cache.clear();
    }

    pub fn size(&self) -> usize {
        self.cache.len()
    }

    pub fn get_account(&self, key: &Vec<u8>) -> Option<Vec<u8>> {
        let snap = &self.db.snapshot();
        let manager = Schema::new(snap);
        manager.state().get(key)
    }

    pub fn get_app_state(&self) -> AppState {
        let snapshot = &self.db.snapshot();
        let schema = Schema::new(snapshot);
        schema.app_state().get().unwrap_or_default()
    }

    fn save(&mut self, k: Vec<u8>, op: WriteOp) {
        self.cache.insert(k, op);
    }
}

//#[cfg(test)]
//mod store_test;

//#[cfg(test)]
//mod schema_test;
