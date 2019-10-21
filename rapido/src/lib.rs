//! Rapido is a Rust framework for building Tendermint applications via ABCI.
//! It provides a high level API to assemble your application with:
//! * Flexible storage options via [Exonum MerkleDb](https://docs.rs/exonum-merkledb)
//! * Elliptic curve crypto via [Exonum Crypto](https://docs.rs/exonum-crypto/)
//! * Deterministic message serialization via [Borsh](http://borsh.io/)
//!
//! This framework is inspired by exonum and other rust based blockchain projects.
pub use self::{
    api::{
        sign_transaction, verify_tx_signature, Service, SignedTransaction, Transaction,
        ValidateTxHandler,
    },
    errors::RapidoError,
};

mod api;
mod appstate;
mod errors;

use abci::*;
use borsh::BorshDeserialize;
use exonum_merkledb::{BinaryValue, Database, Patch};
use std::collections::HashMap;
use std::sync::Arc;

use appstate::{AppState, AppStateStore};

const NAME: &str = "rapido_v1";

// Queries expect this
const REQ_QUERY_PATH_SEPERATOR: &str = "/";
// Reserved Query to return a service hash from the proof table
const RAPIDO_QUERY_ROUTE_APP_HASH: &str = "rapphash/";

/// Use the AppBuilder to assemble an application
pub struct AppBuilder {
    pub db: Arc<dyn Database>,
    pub services: Vec<Box<dyn Service>>,
    pub validate_tx_handler: Option<ValidateTxHandler>,
    pub genesis_data: Option<Vec<u8>>,
}

impl AppBuilder {
    /// Create a new builder with the given Database handle from exonum_merkledb
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self {
            db,
            services: Vec::new(),
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
    pub fn set_validation_handler(mut self, handler: ValidateTxHandler) -> Self {
        self.validate_tx_handler = Some(handler);
        self
    }

    /// Add a Service to the application
    pub fn add_service(mut self, handler: Box<dyn Service>) -> Self {
        self.services.push(handler);
        self
    }

    /// Call to return a configured node. This consumes the underlying builder.
    /// Will panic if no services are set.
    pub fn finish(self) -> Node {
        if self.services.len() == 0 {
            panic!("No services configured!");
        }
        Node::new(self)
    }
}

/// Node provides functionality to execute services and manage storage.  
/// You should use the `AppBuilder` to create a Node.
pub struct Node {
    db: Arc<dyn Database>,
    app_state: AppState,
    services: HashMap<&'static str, Box<dyn Service>>,
    commit_patches: Vec<Patch>,
    validate_tx_handler: Option<ValidateTxHandler>,
    genesis_data: Option<Vec<u8>>,
}

impl Node {
    /// Create a new Node. This is called automatically when using the builder.
    pub fn new(config: AppBuilder) -> Self {
        let db = config.db;

        let mut service_map = HashMap::new();
        for s in config.services {
            let route = s.route();
            // First come, first serve...
            if !service_map.contains_key(route) {
                service_map.insert(route, s);
            }
        }

        Self {
            db: db.clone(),
            app_state: AppState::default(),
            services: service_map,
            commit_patches: Vec::new(),
            validate_tx_handler: config.validate_tx_handler,
            genesis_data: config.genesis_data,
        }
    }

    // internal function called by both check/deliver_tx
    fn run_tx(&mut self, is_check: bool, raw_tx: Vec<u8>) -> Result<(), RapidoError> {
        // Decode the incoming signed transaction
        let tx = SignedTransaction::try_from_slice(&raw_tx[..])?;

        // Return err if there are no services matching the route
        if !self.services.contains_key(&*tx.route) {
            return Err(RapidoError::from(format!(
                "Service not found for route: {}",
                tx.route
            )));
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
        let fork = self.db.fork();
        let service = self.services.get(&*tx.route).expect("should have service");
        let result = service
            .decode_tx(tx.txid, tx.payload.clone())
            .and_then(|handler| handler.execute(tx.sender, &fork));

        // We only save patches from successful transactions
        if result.is_ok() {
            self.commit_patches.push(fork.into_patch());
        }
        result
    }
}

// Parse a query route:  It expects query routes to be in the
// form: 'route/somepath', where 'route' is the name of the service,
// and '/somepath' is your application's specific path. If you
// want to just query on any key, use the form: 'route/'.
fn parse_abci_query_path(req_path: &String) -> Option<(&str, &str)> {
    req_path
        .find(REQ_QUERY_PATH_SEPERATOR)
        .filter(|i| i > &0usize)
        .and_then(|index| Some(req_path.split_at(index)))
}

// Implements the abci::application trait
#[doc(hidden)]
impl abci::Application for Node {
    // Check we're in sync, replay if not...
    fn info(&mut self, req: &RequestInfo) -> ResponseInfo {
        let snapshot = self.db.snapshot();
        let store = AppStateStore::new(&snapshot);
        self.app_state = store.get_commit_info().unwrap_or_default();

        let mut resp = ResponseInfo::new();
        resp.set_data(String::from(NAME));
        resp.set_version(String::from(req.get_version()));
        resp.set_last_block_height(self.app_state.height);
        resp.set_last_block_app_hash(self.app_state.hash.clone());
        resp
    }

    // Ran once on the initial start of the application
    fn init_chain(&mut self, _req: &RequestInitChain) -> ResponseInitChain {
        for (_, service) in &self.services {
            // a little clunky, but only done once
            let fork = self.db.fork();
            let result = service.genesis(&fork, self.genesis_data.as_ref());
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
        let (route, query_path) = match parse_abci_query_path(&req.path) {
            Some(tuple) => tuple,
            None => {
                response.code = 1u32;
                response.key = req.data.clone();
                response.log = "No query path found.  Format should be 'route/apppath'".into();
                return response;
            }
        };

        // Reserved Rapido Query:
        // If the route == RAPIDO_QUERY_ROUTE_APP_HASH, then we query the proof table
        // based on the key to return the last reported state hash for a service. `key`
        // must be a borsh encoded `ProofTableKey`.  AbciQuery should be:
        // path = RAPIDO_QUERY_ROUTE_APP_HASH
        // data = Vec<u8> encoded as a `ProofTableKey`
        //
        // Services can specify their own queries to return a state hashes from their service.
        //
        // You can use this to prove that a specific service hash is as part of the
        // overall apphash formed from the proof table.
        if route == RAPIDO_QUERY_ROUTE_APP_HASH {
            let snapshot = self.db.snapshot();
            let store = AppStateStore::new(&snapshot);
            match store.get_service_hash(key) {
                Some(hash) => {
                    response.code = 0;
                    response.value = hash.to_bytes().to_vec();
                    return response;
                }
                None => {
                    response.code = 1u32;
                    response.log = format!(
                        "Cannot find a proof table state hash for {}. Maybe a bad ProofTableKey?",
                        route
                    );
                    return response;
                }
            }
        }

        // Check if a service exists for this route
        if !self.services.contains_key(route) {
            response.code = 1u32;
            response.log = format!("Cannot find query service for {}", route);
            return response;
        }

        // Call service.query
        let snapshot = self.db.snapshot();
        match self
            .services
            .get(route)
            .unwrap() // <= we unwrap here, because we already checked for it above.
            // So, panic here if something else occurs
            .query(query_path, key, &snapshot)
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
        // Commit accumulated patches from deliverTx to storage and
        // clear commit_patches vec.
        for patch in self.commit_patches.drain(..) {
            self.db.merge(patch).expect("abci:commit patches");
        }

        // Prepare to commit new app state
        let fork = self.db.fork();

        // Collect all the store hashes from each service and add to
        // the appstore tree to calculate a single root for the apphash
        let appstore = AppStateStore::new(&fork);
        for (route, service) in &self.services {
            for (index, hash) in service.store_hashes(&fork).iter().enumerate() {
                appstore.save_service_hash(route, index, hash);
            }
        }
        // New apphash (as bytes) calculated from the proof map
        let state_root_bytes = appstore.get_proof_table_root_hash().to_bytes();

        // Update and commit the new appstate to the store
        let new_height = self.app_state.height + 1;
        appstore.set_commit_info(new_height, state_root_bytes.clone());

        // Update application copy
        self.app_state.hash = state_root_bytes;
        self.app_state.height = new_height;

        // Merge new commits into to db
        // panic here, to let us know there's a problem.
        self.db
            .merge(fork.into_patch())
            .expect("abci:commit appstate");

        let mut resp = ResponseCommit::new();
        resp.set_data(self.app_state.hash.clone());
        resp
    }
}
