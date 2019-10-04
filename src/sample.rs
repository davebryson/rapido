/// Try another DB approach
use std::sync::Arc;

use exonum_merkledb::{Database, Fork, ObjectHash, Patch, ProofMapIndex, TemporaryDB};

pub struct App {
    db: Arc<dyn Database>,
    mempool: Vec<u32>,
    patches: Vec<Patch>,
}

pub struct TxContext {
    pub fork: Fork,
}

impl App {
    pub fn new(db: Arc<dyn Database>) -> Self {
        let db1 = db.clone();
        Self {
            db,
            mempool: Vec::new(),
            patches: Vec::new(),
        }
    }

    pub fn fork(&self) -> Fork {
        self.db.fork()
    }

    // Handler
    pub fn deliver_tx(&mut self, tx: u32, f: &Fork) {
        let mut map: ProofMapIndex<_, u8, u32> = ProofMapIndex::new("sample", f);
        map.put(&1u8, tx);
    }

    pub fn add_tx(&mut self, tx: u32) {
        self.mempool.push(tx);
    }

    pub fn run_txs(&mut self) {
        let temp = self.mempool.clone();
        // Run all the txs...
        for tx in temp {
            let f = self.fork();

            self.deliver_tx(tx, &f);

            self.patches.push(f.into_patch());
        }
    }

    pub fn commit(&mut self) {
        // TODO:  How to collect root hashes from each tree?
        for p in self.patches.drain(..) {
            self.db.merge(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let db = Arc::new(TemporaryDB::new());
        let mut app = App::new(db.clone());

        // Fill mempool
        for i in 1..10000u32 {
            app.add_tx(i)
        }

        {
            app.run_txs();
        }

        {
            app.commit();
        }

        let snap = db.snapshot();
        let map: ProofMapIndex<_, u8, u32> = ProofMapIndex::new("sample", &snap);
        println!("commit hash: {:?}", map.object_hash());
        assert_eq!(9999u32, map.get(&1u8).unwrap());
    }
}
