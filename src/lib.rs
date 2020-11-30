//! Rapido is a Rust framework for building Tendermint applications via ABCI.
//! It provides a high level API to assemble your application with:
//! * Flexible storage options via [Exonum MerkleDb](https://docs.rs/exonum-merkledb)
//! * Elliptic curve crypto via [Exonum Crypto](https://docs.rs/exonum-crypto/)
//! * Deterministic message serialization via [Borsh](http://borsh.io/)
//!
//! This framework is inspired by exonum and other rust based blockchain projects.
pub use self::{
    store::{Store, StoreView},
    types::{
        sign_transaction, verify_tx_signature, AppModule, AuthenticationHandler, Context,
        SignedTransaction,
    },
};

#[macro_use]
mod macros;
mod account;
mod did;
mod schema;
pub mod store;
mod types;

use crate::schema::RapidoSchema;
use abci::*;
use anyhow::bail;
use exonum_merkledb::{Database, Fork, ObjectHash, SystemSchema};
use protobuf::RepeatedField;
use std::collections::HashMap;
use std::sync::Arc;

const NAME: &str = "rapido_v2";
const RESERVED_APP_NAME: &str = "rapido";

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

    pub fn register_apps(mut self, apps: Vec<Box<dyn AppModule>>) -> Self {
        self.appmodules = apps;
        self
    }

    /// Add a Service to the application
    /// TODO: change to register_module()
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
    validate_tx_handler: Option<AuthenticationHandler>,
    genesis_data: Option<Vec<u8>>,
    check_cache: Option<store::Cache>,
    deliver_cache: Option<store::Cache>,
}

impl Node {
    /// Create a new Node. This is called automatically when using the builder.
    pub fn new(config: AppBuilder) -> Self {
        let db = config.db;

        let mut service_map = HashMap::new();
        for s in config.appmodules {
            let route = s.name();
            // Rapido is a reserved app/route name
            if route == RESERVED_APP_NAME {
                panic!("Cannot use app module with the name of 'rapido'. The name is reserved");
            }
            // First come, first serve...
            if !service_map.contains_key(route) {
                service_map.insert(route, s);
            }
        }

        Self {
            db: db.clone(),
            appmodules: service_map,
            validate_tx_handler: config.validate_tx_handler,
            genesis_data: config.genesis_data,
            check_cache: Some(Default::default()),
            deliver_cache: Some(Default::default()),
        }
    }

    // internal function called by both check/deliver_tx
    fn run_tx(
        &mut self,
        is_check: bool,
        raw_tx: Vec<u8>,
    ) -> Result<RepeatedField<Event>, anyhow::Error> {
        // Decode the incoming transaction
        let tx = SignedTransaction::decode(&raw_tx[..])?;

        // Return err if there are no appmodules matching the route
        if !self.appmodules.contains_key(tx.appname()) {
            bail!(format!(
                "No registered Module found for name: {}",
                tx.appname()
            ));
        }

        // If this is a check_tx and a validation handler has been set, run it
        if is_check && self.validate_tx_handler.is_some() {
            let snap = self.db.snapshot();
            let mut cache = store::StoreView::wrap(&snap, self.check_cache.take().unwrap());

            let resp = match self.validate_tx_handler {
                Some(handler) => match handler(&tx, &mut cache) {
                    Ok(()) => Ok(RepeatedField::<Event>::new()),
                    Err(r) => Err(r),
                },
                None => Ok(RepeatedField::new()),
            };
            self.check_cache.replace(cache.into_cache());
            return resp;
        }

        // Run DeliverTx by:
        let app = self.appmodules.get(tx.appname()).expect("app module");
        let snap = self.db.snapshot();
        let mut cache = store::StoreView::wrap(&snap, self.deliver_cache.take().unwrap());

        // TODO: Increment account nonce

        let ctx = tx.into_context();
        let resp = match app.handle_tx(&ctx, &mut cache) {
            Ok(()) => {
                let events = ctx.get_events();
                Ok(events)
            }
            Err(r) => Err(r),
        };

        self.deliver_cache.replace(cache.into_cache());
        resp
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
// form: 'appname/somepath', where 'appname' is the name of the AppModule
// and '/somepath' is your application's specific path. If you
// want to just query on any key, use the form: 'appname/' or 'appname'.
fn parse_abci_query_path(req_path: &str) -> Option<(&str, &str)> {
    if req_path.len() == 0 {
        return None;
    }
    if req_path == "/" {
        return None;
    }
    if !req_path.contains("/") {
        return Some((req_path, "/"));
    }

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
        resp.set_last_block_app_hash(state.apphash.clone());
        resp
    }

    // Ran once on the initial start of the application
    // How to use config like substrate here...?
    // TODO:  Doesn't init_chain call commit?!  If so,
    // Change this to use storeview
    fn init_chain(&mut self, _req: &RequestInitChain) -> ResponseInitChain {
        // a little clunky, but only done once
        for (_, app) in &self.appmodules {
            let fork = self.db.fork();
            let result = app.initialize(&fork, self.genesis_data.as_ref());

            if result.is_err() {
                panic!("problem initializing chain with genesis data");
            }

            if let Err(_) = self.db.merge(fork.into_patch()) {
                panic!("error");
            }
        }
        // THIS SHOULD RETURN INITIAL APP HASH!
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
                response.log = "malformed query path".into();
                return response;
            }
        };

        // Check if a app exists for this name
        if !self.appmodules.contains_key(appname) {
            response.code = 1u32;
            response.log = format!("Query: cannot find appname: {}", appname);
            return response;
        }

        // Call handle_query
        let snapshot = self.db.snapshot();
        let cache = store::StoreView::wrap_snapshot(&snapshot);
        match self
            .appmodules
            .get(appname)
            .unwrap() // <= we unwrap here, because we already checked for it above.
            // So, panic here if something else occurs
            .handle_query(query_path, key, &cache)
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
            Ok(events) => {
                resp.set_code(0);
                resp.events = events;
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
        let snap = self.db.snapshot();
        let cache = store::StoreView::wrap(&snap, self.deliver_cache.take().unwrap());

        let fork = self.db.fork();
        cache.commit(&fork);

        let apphash = self.update_state(&fork);
        self.db
            .merge(fork.into_patch())
            .expect("abci:commit appstate");

        self.deliver_cache.replace(Default::default());
        self.check_cache.replace(Default::default());

        let mut resp = ResponseCommit::new();
        resp.set_data(apphash);
        resp
    }
}
