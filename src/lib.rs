//! Rapido is a Rust framework for building Tendermint applications via ABCI.
//! It provides a high level API to assemble your application with:
//! * Flexible storage options via [Exonum MerkleDb](https://docs.rs/exonum-merkledb)
//! * Elliptic curve crypto via [Exonum Crypto](https://docs.rs/exonum-crypto/)
//! * Deterministic message serialization via [Borsh](http://borsh.io/)
//!
//! This framework is inspired by exonum and other rust based blockchain projects.
pub use self::types::{
    sign_transaction, verify_tx_signature, AppModule, AuthenticationHandler, Context,
    SignedTransaction,
};

mod account;
mod did;
mod schema;
mod types;

use crate::schema::RapidoSchema;
//use crate::types::Context;
use abci::*;
use anyhow::bail;
use borsh::BorshDeserialize;
use exonum_merkledb::{Database, Fork, ObjectHash, Patch, SystemSchema};
use std::collections::HashMap;
use std::sync::Arc;

const NAME: &str = "rapido_v2";

/// Use the AppBuilder to assemble an application
pub struct AppBuilder {
    pub db: Arc<dyn Database>,
    pub appmodules: Vec<Box<dyn AppModule>>,
    pub validate_tx_handler: Option<AuthenticationHandler>,
    pub genesis_data: Option<Vec<u8>>,
}

impl AppBuilder {
    /// Create a new builder with the given Database handle from exonum_merkledb
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self {
            db,
            appmodules: Vec::new(),
            validate_tx_handler: None,
            genesis_data: None,
        }
    }

    /// Set genesis data to be used on initial startup. How the data
    /// is interpreted is left to the Service implementation
    pub fn set_genesis_data(mut self, data: Vec<u8>) -> Self {
        if data.len() > 0 {
            self.genesis_data = Some(data);
        }
        self
    }

    /// Set the desired validation handler. If not set, checkTx will return 'ok' by default
    pub fn set_validation_handler(mut self, handler: AuthenticationHandler) -> Self {
        self.validate_tx_handler = Some(handler);
        self
    }

    /// Add a Service to the application
    pub fn add_service(mut self, handler: Box<dyn AppModule>) -> Self {
        self.appmodules.push(handler);
        self
    }

    /// Call to return a configured node. This consumes the underlying builder.
    /// Will panic if no appmodules are set.
    pub fn finish(self) -> Node {
        if self.appmodules.len() == 0 {
            panic!("No appmodules configured!");
        }
        Node::new(self)
    }
}

/// Node provides functionality to execute appmodules and manage storage.  
/// You should use the `AppBuilder` to create a Node.
pub struct Node {
    db: Arc<dyn Database>,
    appmodules: HashMap<&'static str, Box<dyn AppModule>>,
    commit_patches: Vec<Patch>,
    validate_tx_handler: Option<AuthenticationHandler>,
    genesis_data: Option<Vec<u8>>,
}

impl Node {
    /// Create a new Node. This is called automatically when using the builder.
    pub fn new(config: AppBuilder) -> Self {
        let db = config.db;

        let mut service_map = HashMap::new();
        for s in config.appmodules {
            let route = s.name();
            // First come, first serve...
            if !service_map.contains_key(route) {
                service_map.insert(route, s);
            }
        }

        Self {
            db: db.clone(),
            appmodules: service_map,
            commit_patches: Vec::new(),
            validate_tx_handler: config.validate_tx_handler,
            genesis_data: config.genesis_data,
        }
    }

    // internal function called by both check/deliver_tx
    fn run_tx(&mut self, is_check: bool, raw_tx: Vec<u8>) -> Result<(), anyhow::Error> {
        // Decode the incoming signed transaction
        let tx = SignedTransaction::try_from_slice(&raw_tx[..])?;

        // Return err if there are no appmodules matching the route
        if !self.appmodules.contains_key(&*tx.app) {
            bail!(format!("AppMoudule not found for name: {}", tx.app));
        }

        // If this is a check_tx and a validation handler has been
        // set, run it
        if is_check {
            // NOTE:  checkTx has read-only to store
            let snapshot = self.db.snapshot();
            return match self.validate_tx_handler {
                Some(handler) => handler(&tx, &snapshot),
                None => Ok(()),
            };
        }

        // Run DeliverTx by:
        // - Getting the service based on the signed transaction 'route'
        // - Decoding the message in the signed transaction
        // - executing the associated Transaction
        let mut fork = self.db.fork();
        let app = self.appmodules.get(&*tx.app).expect("app module");

        // TODO: Increment account nonce

        let ctx = Context::from_tx(tx, &mut fork);
        let result = app.handle_tx(ctx);
        if result.is_ok() {
            self.db.merge(fork.into_patch())?;
        }
        result
    }

    // Called in abci.commit
    fn update_state(&mut self, fork: &Fork) -> Vec<u8> {
        let aggregator = SystemSchema::new(fork).state_aggregator();
        let statehash = aggregator.object_hash().as_bytes().to_vec();

        let mut rapidostate = RapidoSchema::new(fork);
        let laststate = rapidostate.get_chain_state().unwrap_or_default();
        let new_height = laststate.height + 1;
        rapidostate.save_chain_state(new_height, statehash.clone());
        statehash.clone()
    }
}

// Parse a query route:  It expects query routes to be in the
// form: 'route/somepath', where 'route' is the name of the service,
// and '/somepath' is your application's specific path. If you
// want to just query on any key, use the form: 'route/'.
fn parse_abci_query_path(req_path: &String) -> Option<(&str, &str)> {
    req_path
        .find("/")
        .filter(|i| i > &0usize)
        .and_then(|index| Some(req_path.split_at(index)))
}

// Implements the abci::application trait
#[doc(hidden)]
impl abci::Application for Node {
    // Check we're in sync, replay if not...
    fn info(&mut self, req: &RequestInfo) -> ResponseInfo {
        let snapshot = self.db.snapshot();
        let store = RapidoSchema::new(&snapshot);
        let state = store.get_chain_state().unwrap_or_default();

        let mut resp = ResponseInfo::new();
        resp.set_data(String::from(NAME));
        resp.set_version(String::from(req.get_version()));
        resp.set_last_block_height(state.height);
        resp.set_last_block_app_hash(state.hash.clone());
        resp
    }

    // Ran once on the initial start of the application
    fn init_chain(&mut self, _req: &RequestInitChain) -> ResponseInitChain {
        for (_, app) in &self.appmodules {
            // a little clunky, but only done once
            let fork = self.db.fork();
            let result = app.initialize(&fork, self.genesis_data.as_ref());
            if result.is_ok() {
                // We only save patches from successful transactions
                self.commit_patches.push(fork.into_patch());
            }
        }
        ResponseInitChain::new()
    }

    fn query(&mut self, req: &RequestQuery) -> ResponseQuery {
        let mut response = ResponseQuery::new();
        let key = req.data.clone();

        // Parse the path.  See `parse_abci_query_path` for the requirements
        let (appname, query_path) = match parse_abci_query_path(&req.path) {
            Some(tuple) => tuple,
            None => {
                response.code = 1u32;
                response.key = req.data.clone();
                response.log = "No query path found.  Format should be 'route/apppath'".into();
                return response;
            }
        };

        // Check if a app exists for this name
        if !self.appmodules.contains_key(appname) {
            response.code = 1u32;
            response.log = format!("Cannot find query for appname: {}", appname);
            return response;
        }

        // Call handle_query
        let snapshot = self.db.snapshot();
        match self
            .appmodules
            .get(appname)
            .unwrap() // <= we unwrap here, because we already checked for it above.
            // So, panic here if something else occurs
            .handle_query(query_path, key, &snapshot)
        {
            Ok(value) => {
                response.code = 0;
                response.value = value;
                response.key = req.data.clone();
                response
            }
            Err(msg) => {
                response.code = 1u32;
                response.key = req.data.clone();
                response.set_log(msg.to_string());
                response
            }
        }
    }

    fn check_tx(&mut self, req: &RequestCheckTx) -> ResponseCheckTx {
        let mut resp = ResponseCheckTx::new();
        match self.run_tx(true, req.tx.clone()) {
            Ok(_) => {
                resp.set_code(0);
                resp
            }
            Err(msg) => {
                resp.set_code(1u32);
                resp.set_log(msg.to_string());
                resp
            }
        }
    }

    fn deliver_tx(&mut self, req: &RequestDeliverTx) -> ResponseDeliverTx {
        let mut resp = ResponseDeliverTx::new();
        match self.run_tx(false, req.tx.clone()) {
            Ok(_) => {
                resp.set_code(0);
                resp
            }
            Err(msg) => {
                resp.set_code(1u32);
                resp.set_log(msg.to_string());
                resp
            }
        }
    }

    fn begin_block(&mut self, _req: &RequestBeginBlock) -> ResponseBeginBlock {
        ResponseBeginBlock::new()
    }

    fn end_block(&mut self, _req: &RequestEndBlock) -> ResponseEndBlock {
        // Should do validator updates
        ResponseEndBlock::new()
    }

    fn commit(&mut self, _req: &RequestCommit) -> ResponseCommit {
        let fork = self.db.fork();

        let apphash = self.update_state(&fork);
        self.db
            .merge(fork.into_patch())
            .expect("abci:commit appstate");

        let mut resp = ResponseCommit::new();
        resp.set_data(apphash);
        resp
    }
}
