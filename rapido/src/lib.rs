//! Rapido is a Rust framework for building Tendermint applications via ABCI.
//! It provides a high level API to assemble your application with:
//! * flexible storage options via [Exonum MerkleDb](https://docs.rs/exonum-merkledb)
//! * Elliptic curve crypto via [Exonum Crypto](https://docs.rs/exonum-crypto/)
//! * deterministic message serialization via [Borsh](http://borsh.io/)
//!

pub use self::{
    account_address::{AccountAddress, ACCT_ADDRESS_LENGTH},
    api::{
        sign_transaction, verify_tx_signature, QueryResult, Service, SignedTransaction,
        Transaction, TxResult, ValidateTxHandler,
    },
};

mod account_address;
mod api;
mod appstate;

use abci::*;
use borsh::BorshDeserialize;
use exonum_crypto::Hash;
use exonum_merkledb::{BinaryValue, Database, Patch};
use std::collections::HashMap;
use std::sync::Arc;

use appstate::{AppState, AppStateSchema};

const NAME: &str = "rapido_v1";
const REQ_QUERY_PATH_SEPERATOR: &str = "/";

/// Use the AppBuilder to assemble an application
pub struct AppBuilder {
    pub db: Arc<dyn Database>,
    pub services: Vec<Box<dyn Service>>,
    pub validate_tx_handler: Option<ValidateTxHandler>,
}

impl AppBuilder {
    /// Create a new builder with the given Database handle
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self {
            db,
            services: Vec::new(),
            validate_tx_handler: None,
        }
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

/// abci result code: Service was not found
pub const SERVICE_NOT_FOUND: u32 = 100;
/// abci result code: Error decoding the signed transaction
pub const TXERR_SIGNED_TX: u32 = 101;
/// abci result code: Error decoding the application transaction/message
pub const TXERR_DECODE_TX: u32 = 102;
/// abci result code: No route found for the query
pub const QUERYERR_NO_ROUTE: u32 = 103;

/// Node provides functionality to execute services and manage storage.  
/// You should use the `AppBuilder` to create a Node.
pub struct Node {
    db: Arc<dyn Database>,
    app_state: AppState,
    services: HashMap<&'static str, Box<dyn Service>>,
    commit_patches: Vec<Patch>,
    validate_tx_handler: Option<ValidateTxHandler>,
}

impl Node {
    /// Create a new Node. This is called automatically when using the builder.
    pub fn new(config: AppBuilder) -> Self {
        let db = config.db;

        //let mut service_map: HashMap<String, Box<dyn Service>> = HashMap::new();
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
        }
    }

    // internal function called by both check/deliver_tx
    fn run_tx(&mut self, is_check: bool, raw_tx: Vec<u8>) -> TxResult {
        let tx = match SignedTransaction::try_from_slice(&raw_tx[..]) {
            Ok(tx) => tx,
            Err(e) => {
                return TxResult::error(
                    TXERR_SIGNED_TX,
                    format!("Err parsing SignedTransaction: {:?}", &e),
                )
            }
        };

        // Return err if there are no services matching the route
        if !self.services.contains_key(&*tx.route) {
            return TxResult::error(
                SERVICE_NOT_FOUND,
                format!("Service not found for route: {}", tx.route),
            );
        }

        if is_check {
            // NOTE:  checkTx has read-only to store
            let snapshot = self.db.snapshot();
            return match self.validate_tx_handler {
                Some(handler) => handler(&tx, &snapshot),
                None => TxResult::ok(),
            };
        }

        // Run DeliverTx
        let fork = self.db.fork();
        let result = match self
            .services
            .get(&*tx.route)
            .and_then(|s| s.decode_tx(tx.txid, tx.payload.clone()).ok())
        {
            // Execute the STF
            Some(handler) => handler.execute(tx.sender, &fork),
            None => return TxResult::error(TXERR_DECODE_TX, "Err decoding transaction"),
        };

        if result.code == 0 {
            // We only save patches from successful transactions
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
        let schema = AppStateSchema::new(&snapshot);
        self.app_state = schema.app_state().get().unwrap_or_default();

        let mut resp = ResponseInfo::new();
        resp.set_data(String::from(NAME));
        resp.set_version(String::from(req.get_version()));
        resp.set_last_block_height(self.app_state.version);
        resp.set_last_block_app_hash(self.app_state.hash.clone());
        resp
    }

    // Ran once on the initial start of the application
    fn init_chain(&mut self, _req: &RequestInitChain) -> ResponseInitChain {
        for (_, service) in &self.services {
            // a little clunky, but only done once
            let fork = self.db.fork();
            let result = service.genesis(&fork);
            if result.code == 0 {
                // We only save patches from successful transactions
                self.commit_patches.push(fork.into_patch());
            }
        }
        ResponseInitChain::new()
    }

    fn query(&mut self, req: &RequestQuery) -> ResponseQuery {
        let mut response = ResponseQuery::new();
        let key = req.data.clone();

        // Parse the path.  See `parse_abci_query_path` for requirements
        let (route, query_path) = match parse_abci_query_path(&req.path) {
            Some(tuple) => tuple,
            None => {
                response.code = QUERYERR_NO_ROUTE;
                response.key = req.data.clone();
                response.log = "No query path found.  Format should be 'route/apppath'".into();
                return response;
            }
        };

        // Check if a service exists for this route
        //let route_as_string: &String = &route.into();
        if !self.services.contains_key(route) {
            response.code = SERVICE_NOT_FOUND;
            response.log = format!("cannot find query service for {}", route);
            return response;
        }

        // Call service.query
        let snapshot = self.db.snapshot();
        let result = self
            .services
            .get(route)
            .unwrap() // <= we unwrap here, because we already checked for it above.
            // So, panic here if something else occurs
            .query(query_path, key, &snapshot);

        // Return the result
        response.code = result.code;
        response.value = result.value;
        response.key = req.data.clone();
        response
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
        // Should do validator updates
        ResponseEndBlock::new()
    }

    fn commit(&mut self, _req: &RequestCommit) -> ResponseCommit {
        // Commit accumulated patches from deliverTx to storage and clear commit_patches vec.
        for patch in self.commit_patches.drain(..) {
            self.db.merge(patch).expect("abci:commit patches");
        }

        let fork = self.db.fork();

        // Calculate new app hash from all services
        let mut hashes: Vec<Hash> = Vec::new();
        for (_, service) in &self.services {
            hashes.push(service.root_hash(&fork));
        }
        let state_root = exonum_merkledb::root_hash(&hashes);

        // Update and commit the app state
        let commit_schema = AppStateSchema::new(&fork);
        self.app_state.hash = state_root.to_bytes();
        self.app_state.version = self.app_state.version + 1;
        commit_schema.app_state().set(AppState {
            version: self.app_state.version,
            hash: self.app_state.hash.clone(),
        });

        // Merge new commits into to db
        self.db
            .merge(fork.into_patch())
            .expect("abci:commit appstate");

        let mut resp = ResponseCommit::new();
        resp.set_data(self.app_state.hash.clone());
        resp
    }
}

#[cfg(test)]
mod appstate_test;

#[cfg(test)]
mod api_test;

#[cfg(test)]
mod abci_test;
