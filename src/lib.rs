//! Rapido is a Rust framework for building Tendermint applications via ABCI.
//! It provides a high level API to assemble your application with:
//! * Flexible storage options via [Exonum MerkleDb](https://docs.rs/exonum-merkledb)
//! * Elliptic curve crypto via [Exonum Crypto](https://docs.rs/exonum-crypto/)
//! * Deterministic message serialization via [Borsh](http://borsh.io/)
//!
//! This framework is inspired by exonum and other rust based blockchain projects.

#[macro_use]
mod macros;
pub mod account;
mod auth;
pub mod client;
mod schema;
mod store;
mod testkit;
mod types;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::schema::RapidoSchema;
use abci::*;
use anyhow::{bail, ensure};
use env_logger::Env;
use exonum_merkledb::{Database, DbOptions, Fork, ObjectHash, RocksDB, SystemSchema, TemporaryDB};
use protobuf::RepeatedField;

// Re-export
pub use self::{
    store::{Store, StoreView},
    testkit::TestKit,
    types::{
        sign_transaction, verify_tx_signature, AppModule, Authenticator, Context, SignedTransaction,
    },
};

const NAME: &str = "rapido_v3";
const RESERVED_APP_NAME: &str = "rapido";
const RAPIDO_HOME: &str = ".rapido";
const RAPIDO_STATE_DIR: &str = "state";

fn dbdir() -> PathBuf {
    let mut dir = dirs::home_dir().expect("find home dir");
    dir.push(RAPIDO_HOME);
    dir.push(RAPIDO_STATE_DIR);
    dir
}

/// Use the AppBuilder to assemble an application
pub struct AppBuilder {
    db: Arc<dyn Database>,
    appmodules: Vec<Box<dyn AppModule>>,
    validate_tx_handler: Option<Box<dyn Authenticator>>,
    use_rocks_db: bool,
}

impl AppBuilder {
    pub fn new() -> Self {
        Self {
            db: Arc::new(TemporaryDB::new()),
            appmodules: Vec::new(),
            validate_tx_handler: None,
            use_rocks_db: false,
        }
    }

    pub fn use_production_db(mut self) -> Self {
        self.use_rocks_db = true;
        self
    }

    /// Set the desired validation handler. If not set, checkTx will return 'ok' by default
    pub fn set_authenticator(mut self, authenticator: impl Into<Box<dyn Authenticator>>) -> Self {
        self.validate_tx_handler = Some(authenticator.into());
        self
    }

    pub fn with_app(mut self, app: impl Into<Box<dyn AppModule>>) -> Self {
        self.appmodules.push(app.into());
        self
    }

    /// Call to return a configured node with a Temp/in-memory db
    /// Use to directly interact with ABCI calls during development
    pub fn node(self) -> Node {
        if self.appmodules.len() == 0 {
            panic!("No appmodules configured!");
        }
        Node::new(self)
    }

    pub fn run(mut self) {
        env_logger::Builder::from_env(Env::default().default_filter_or("info"))
            .try_init()
            .expect("logger");

        if self.appmodules.len() == 0 {
            panic!("No appmodules configured!");
        }

        if self.use_rocks_db {
            let db = RocksDB::open(dbdir(), &DbOptions::default()).expect("create rocks db");
            self.db = Arc::new(db);
        }

        let node = Node::new(self);
        abci::run_local(node);
    }
}

/// Node provides functionality to execute appmodules and manage storage.  
/// You should use the `AppBuilder` to create a Node.
pub struct Node {
    db: Arc<dyn Database>,
    appmodules: HashMap<&'static str, Box<dyn AppModule>>,
    authenticator: Box<dyn Authenticator>,
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

        // Use the default authenticator if one is not set.
        let auth = match config.validate_tx_handler {
            Some(a) => a,
            None => Box::new(auth::DefaultAuthenticator),
        };

        Self {
            db: db.clone(),
            appmodules: service_map,
            authenticator: auth,
            check_cache: Some(Default::default()),
            deliver_cache: Some(Default::default()),
        }
    }

    // internal function called by both check/deliver_tx
    fn run_tx(
        &mut self,
        is_check: bool,
        raw_tx: Vec<u8>,
    ) -> anyhow::Result<RepeatedField<Event>, anyhow::Error> {
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
        if is_check {
            let snap = self.db.snapshot();
            let mut cache = store::StoreView::wrap(&snap, self.check_cache.take().unwrap());

            let resp = match self.authenticator.validate(&tx, &cache) {
                Ok(()) => Ok(RepeatedField::<Event>::new()),
                Err(r) => Err(r),
            };

            // Increment the nonce for a sender in the checkTx cache
            ensure!(
                self.authenticator.increment_nonce(&tx, &mut cache).is_ok(),
                "check tx nonce error"
            );

            self.check_cache.replace(cache.into_cache());
            return resp;
        }

        // Run DeliverTx by:
        let app = self.appmodules.get(tx.appname()).expect("app module");
        let snap = self.db.snapshot();
        let mut cache = store::StoreView::wrap(&snap, self.deliver_cache.take().unwrap());

        let ctx = tx.into_context();
        let resp = match app.handle_tx(&ctx, &mut cache) {
            Ok(()) => {
                let events = ctx.get_events();
                Ok(events)
            }
            Err(r) => Err(r),
        };

        // Increment the nonce for a sender
        ensure!(
            self.authenticator.increment_nonce(&tx, &mut cache).is_ok(),
            "deliver tx nonce error"
        );

        self.deliver_cache.replace(cache.into_cache());
        resp
    }

    // Called by abci.commit
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

    // Ran once on the initial start of the application.
    // AppModules can implement `initialize` to load initial state.
    fn init_chain(&mut self, _req: &RequestInitChain) -> ResponseInitChain {
        let snap = self.db.snapshot();
        let mut cache = store::StoreView::wrap(&snap, self.deliver_cache.take().unwrap());

        for (_, app) in &self.appmodules {
            let result = app.initialize(&mut cache);

            if result.is_err() {
                panic!("problem initializing chain with genesis data");
            }
        }

        // TODO: Put validators in state
        self.deliver_cache.replace(cache.into_cache());
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
                response.log = "Malformed query path".into();
                return response;
            }
        };

        let snapshot = self.db.snapshot();
        let cache = store::StoreView::wrap_snapshot(&snapshot);

        // TODO: Add rapdio queries:
        // /rapido/apphash
        // /rapido/validators

        // Check if a app exists for this name
        if !self.appmodules.contains_key(appname) {
            response.code = 1u32;
            response.log = format!("Query: cannot find appname: {}", appname);
            return response;
        }

        // Call handle_query
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
        // do validator updates
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
