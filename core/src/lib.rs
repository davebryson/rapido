//! Rapido is a Rust framework for building Tendermint applications.
//! It provides a high level API to assemble your application with:
//! * Merkle Tree storage via [Exonum MerkleDb](https://docs.rs/exonum-merkledb)
//! * Elliptic curve crypto via [Exonum Crypto](https://docs.rs/exonum-crypto/)
//! * Deterministic message serialization via [Borsh](http://borsh.io/)
#[macro_use]
mod macros;
mod schema;
mod store;
mod testkit;
mod types;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[macro_use]
extern crate log;

use crate::schema::RapidoSchema;
use abci::*;
use anyhow::{bail, ensure};
use env_logger::Env;
use exonum_merkledb::{Database, DbOptions, Fork, ObjectHash, RocksDB, SystemSchema, TemporaryDB};
use protobuf::RepeatedField;

// Re-export
pub use self::{
    store::{Store, StoreView},
    testkit::{testing_keypair, TestKit},
    types::{
        sign_transaction, verify_tx_signature, AccountId, AppModule, Authenticator, Context,
        SignedTransaction,
    },
};

const NAME: &str = "rapido_v3";
const RESERVED_APP_NAME: &str = "rapido";
const RAPIDO_HOME: &str = ".rapido";
const RAPIDO_STATE_DIR: &str = "state";

// Create a directory for rocksdb at ~/home/.rapido/state
fn dbdir() -> PathBuf {
    let mut dir = dirs::home_dir().expect("find home dir");
    dir.push(RAPIDO_HOME);
    dir.push(RAPIDO_STATE_DIR);
    dir
}

/// Assemble your app.
/// Example:
/// ```ignore
///  AppBuilder::new().with_app(MyModule {}).run();
/// ```
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

    /// Add this call to the use rockdb to persist application state.
    /// By default a temp in-memory db is used.
    pub fn use_production_db(mut self) -> Self {
        self.use_rocks_db = true;
        self
    }

    /// Set an Authentication handler. See the `Authenticator` trait to implement
    /// you own. If an authenticator is not set, the default is used with no tx authentication.
    pub fn set_authenticator(mut self, authenticator: impl Into<Box<dyn Authenticator>>) -> Self {
        self.validate_tx_handler = Some(authenticator.into());
        self
    }

    /// Call this one or more times to add AppModules to the overall App.
    pub fn with_app(mut self, app: impl Into<Box<dyn AppModule>>) -> Self {
        self.appmodules.push(app.into());
        self
    }

    /// Call to return a configured node with a temp/in-memory db
    /// Use to directly interact with ABCI calls during development.
    pub fn node(self) -> Node {
        if self.appmodules.len() == 0 {
            panic!("No appmodules configured!");
        }
        Node::new(self)
    }

    /// Called last to start the application via rust-abci.  This will start
    /// the application and connect to Tendermint.
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
        info!(" ~~ starting application ~~");
        info!(" ... waiting for connection from Tendermint ...");
        abci::run_local(node);
    }
}

/// Default authenticator used if one is not set in the AppBuilder.
/// Returns Ok for any Tx. and does not increment a nonce.
pub struct DefaultAuthenticator;
impl Authenticator for DefaultAuthenticator {
    fn validate(&self, _tx: &SignedTransaction, _view: &StoreView) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

#[doc(hidden)]
/// Node provides functionality to execute appmodules and manage storage.  
/// You should use the `AppBuilder` to create a Node.
pub struct Node {
    db: Arc<dyn Database>,
    appmodules: HashMap<String, Box<dyn AppModule>>,
    authenticator: Box<dyn Authenticator>,
    check_cache: Option<store::Cache>,
    deliver_cache: Option<store::Cache>,
}

impl Node {
    /// Create a new Node. This is called automatically when using the builder.
    pub fn new(config: AppBuilder) -> Self {
        let db = config.db;

        // setup route mapping
        let mut service_map = HashMap::new();
        for s in config.appmodules {
            let route = s.name();
            // Rapido is a reserved app/route name
            if route == RESERVED_APP_NAME {
                panic!("The AppModule name 'rapido' is reserved for internal use");
            }
            // First come, first serve...
            if !service_map.contains_key(&route) {
                service_map.insert(route, s);
            }
        }

        // Use the default authenticator if one is not set.
        let auth = match config.validate_tx_handler {
            Some(a) => a,
            None => Box::new(DefaultAuthenticator),
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
                "No registered AppModule found for name: {}",
                tx.appname()
            ));
        }

        // If this is a check_tx handle it
        if is_check {
            let snap = self.db.snapshot();
            let mut cache = store::StoreView::wrap(&snap, self.check_cache.take().unwrap());

            let resp = match self.authenticator.validate(&tx, &cache) {
                Ok(()) => Ok(RepeatedField::<Event>::new()),
                Err(r) => Err(r),
            };

            // Increment the nonce for a sender in the checkTx cache
            // this is to ensure multiple txs from a user are tracked
            // this doesn't affect the nonce count in deliver_tx
            ensure!(
                self.authenticator.increment_nonce(&tx, &mut cache).is_ok(),
                "check tx : inc nonce error"
            );

            // Refresh the cache
            self.check_cache.replace(cache.into_cache());
            // We're done here...
            return resp;
        }

        // Run DeliverTx

        // Expect shouldn't ever happen. We checked above
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

    // Called by abci.commit() below
    fn update_state(&mut self, fork: &Fork) -> Vec<u8> {
        // Use the root aggregator from Exonum.
        // Probably don't really need this as we're using 1
        // tree for all...
        let aggregator = SystemSchema::new(fork).state_aggregator();
        let statehash = aggregator.object_hash().as_bytes().to_vec();

        // Update the Rapido chain state
        let mut rapidostate = RapidoSchema::new(fork);
        let laststate = rapidostate.get_chain_state().unwrap_or_default();
        let new_height = laststate.height + 1;
        rapidostate.save_chain_state(new_height, statehash.clone());
        // Return the new apphash
        statehash.clone()
    }
}

// Parse a query route:  It expects query routes to be in the
// form: 'appname/somepath', where 'appname' is the name of the AppModule
// and '/somepath' is your application's specific path. If you
// want to just query on any key, use the form: 'appname/' or 'appname'.
// Returns (appname, path remainder)
fn parse_abci_query_path(req_path: &str) -> Option<(&str, &str)> {
    // Need a path...
    if req_path.len() == 0 {
        return None;
    }
    // Need an appname
    if req_path == "/" {
        return None;
    }
    // Add a '/' if one not provided for consistency
    if !req_path.contains("/") {
        return Some((req_path, "/"));
    }

    // Find the first '/' and parse from there...
    req_path
        .find("/")
        .filter(|i| i > &0usize)
        .and_then(|index| Some(req_path.split_at(index)))
}

// Implements the abci::application trait
#[doc(hidden)]
impl abci::Application for Node {
    // Check we're in sync, replay if not... called on startup
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

    // Ran once on the initial (genesis) of the application.
    // AppModules can implement `initialize` to load their own initial state.
    fn init_chain(&mut self, _req: &RequestInitChain) -> ResponseInitChain {
        let snap = self.db.snapshot();
        let mut cache = store::StoreView::wrap(&snap, self.deliver_cache.take().unwrap());

        for (_, app) in &self.appmodules {
            let result = app.initialize(&mut cache);

            if result.is_err() {
                panic!("problem initializing chain with genesis data");
            }
        }

        let fork = self.db.fork();
        cache.commit(&fork);
        //let aggregator = SystemSchema::new(&fork).state_aggregator();
        //let statehash = aggregator.object_hash().as_bytes().to_vec();
        //let resp = ResponseInitChain::new();
        self.db.merge(fork.into_patch()).expect("init_chain:commit");

        // TODO: Put validators in state
        self.deliver_cache.replace(Default::default());
        ResponseInitChain::new()
    }

    // handle rpc queries
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

        // TODO: Add rapdio reserved queries:
        // /rapido/apphash
        // /rapido/validators

        // Check if a app exists for this name
        if !self.appmodules.contains_key(appname) {
            response.code = 1u32;
            response.log = format!("Query: cannot find appname: {}", appname);
            return response;
        }

        // Call AppModule handle_query
        // We return 0 if all is bueno, else 1
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

    // Who gets in the Tendermint mempool...?
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

    // Well you made is this far, let's see if you can influence app state.
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

    // Commit the txs and update app state
    fn commit(&mut self, _req: &RequestCommit) -> ResponseCommit {
        let snap = self.db.snapshot();
        let cache = store::StoreView::wrap(&snap, self.deliver_cache.take().unwrap());

        let fork = self.db.fork();
        cache.commit(&fork);

        // new state root hash!
        let apphash = self.update_state(&fork);
        self.db
            .merge(fork.into_patch())
            .expect("abci:commit appstate");

        // Refresh the caches
        self.deliver_cache.replace(Default::default());
        self.check_cache.replace(Default::default());

        let mut resp = ResponseCommit::new();
        resp.set_data(apphash);
        resp
    }
}
