pub use self::{
    builder::AppBuilder,
    traits::{FromProtoBytes, IntoProtoBytes, TxHandler},
    tx::Tx,
    types::{TxContext, TxResult, ValidateTxHandler},
};

mod builder;
mod traits;
mod tx;
mod types;

use abci::*;
use exonum_merkledb::Database;
use std::collections::HashMap;
use std::sync::Arc;

use crate::store::{AppState, Schema, StateStore};

const NAME: &str = "rapido_v1";

pub struct Node {
    db: Arc<dyn Database>,
    check_cache: StateStore,
    deliver_cache: StateStore,
    app_state: AppState,
    handlers: HashMap<String, Box<dyn TxHandler>>,
    validate_tx_handler: Option<ValidateTxHandler>,
}

impl Node {
    pub fn new(config: AppBuilder) -> Self {
        let db = config.db;

        let mut map: HashMap<String, Box<dyn TxHandler>> = HashMap::new();
        for h in config.handlers {
            let route = h.route();
            map.insert(route, h);
        }

        Self {
            db: db.clone(),
            check_cache: StateStore::new(db.clone()),
            deliver_cache: StateStore::new(db.clone()),
            app_state: AppState::default(),
            handlers: map,
            validate_tx_handler: config.validate_tx_handler,
        }
    }

    fn run_tx(&mut self, is_check: bool, raw_tx: Vec<u8>) -> TxResult {
        let tx = match Tx::from_proto_bytes(&raw_tx[..]) {
            Ok(tx) => tx,
            Err(e) => return TxResult::error(11, format!("Err parsing Tx: {:?}", &e)),
        };

        // Return err if there are no handlers matching the route
        if !self.handlers.contains_key(&tx.route) {
            return TxResult::error(12, format!("Handler not found for route: {}", tx.route));
        }

        if is_check {
            return match self.validate_tx_handler {
                Some(handler) => handler(TxContext::new(&mut self.check_cache, &tx)),
                None => TxResult::ok(),
            };
        }

        let handler = self.handlers.get(&tx.route).unwrap();
        handler.execute(TxContext::new(&mut self.deliver_cache, &tx))
    }
}

impl abci::Application for Node {
    fn info(&mut self, req: &RequestInfo) -> ResponseInfo {
        let snapshot = self.db.snapshot();
        let schema = Schema::new(&snapshot);
        self.app_state = schema.app_state().get().unwrap_or_default();

        let mut resp = ResponseInfo::new();
        resp.set_data(String::from(NAME));
        resp.set_version(String::from(req.get_version()));
        resp.set_last_block_height(self.app_state.version);
        resp.set_last_block_app_hash(self.app_state.hash.clone());
        resp
    }

    fn init_chain(&mut self, _req: &RequestInitChain) -> ResponseInitChain {
        ResponseInitChain::new()
    }

    fn check_tx(&mut self, req: &RequestCheckTx) -> ResponseCheckTx {
        self.run_tx(true, req.tx.clone()).into()
    }

    fn deliver_tx(&mut self, req: &RequestDeliverTx) -> ResponseDeliverTx {
        self.run_tx(false, req.tx.clone()).into()
    }

    fn begin_block(&mut self, _req: &RequestBeginBlock) -> ResponseBeginBlock {
        ResponseBeginBlock::new()
    }

    fn end_block(&mut self, _req: &RequestEndBlock) -> ResponseEndBlock {
        ResponseEndBlock::new()
    }

    fn commit(&mut self, _req: &RequestCommit) -> ResponseCommit {
        let mut resp = ResponseCommit::new();

        // get mutable access to the db
        let fork = self.db.fork();
        // Commit write batch and get new root hash
        let root_hash = self.deliver_cache.commit(&fork);
        // clear the check cache
        self.check_cache.reset_cache();

        // Update app state
        self.app_state.hash = root_hash.clone();
        self.app_state.version = self.app_state.version + 1;

        // Commit app state
        let commit_schema = Schema::new(&fork);
        commit_schema.app_state().set(AppState {
            version: self.app_state.version,
            hash: self.app_state.hash.clone(),
        });

        // Merge to db
        self.db.merge(fork.into_patch()).unwrap();

        resp.set_data(root_hash);
        resp
    }
}
