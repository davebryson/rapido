///!
///! Rapido is a Rust framework for building Tendermint applications via ABCI.
///!
pub use self::api::{
    sign_transaction, verify_tx_signature, AccountId, QueryResult, Service, SignedTransaction,
    Transaction, TxResult, ValidateTxHandler,
};

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
const REQ_QUERY_PATH_SEPERATOR: &str = "**";

/// Builder to assemble an application
pub struct AppBuilder {
    pub db: Arc<dyn Database>,
    pub services: Vec<Box<dyn Service>>,
    pub validate_tx_handler: Option<ValidateTxHandler>,
}

impl AppBuilder {
    // Create a new builder with a Database handle
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self {
            db,
            services: Vec::new(),
            validate_tx_handler: None,
        }
    }
    // Set the desired validation handler. If not set,
    // checkTx will return 'ok' by default
    pub fn set_validation_handler(mut self, handler: ValidateTxHandler) -> Self {
        self.validate_tx_handler = Some(handler);
        self
    }

    // Add a Service to the application
    pub fn add_service(mut self, handler: Box<dyn Service>) -> Self {
        self.services.push(handler);
        self
    }

    // Call to return a configured node. This consumes the underlying builder.
    // Will panic if no services are set.
    pub fn finish(self) -> Node {
        if self.services.len() == 0 {
            panic!("No services configured!");
        }
        Node::new(self)
    }
}

// abci result codes used by the node
pub const TXERR_CODE_SIGNED_TX: u32 = 100;
pub const TXERR_CODE_SERVICE_NOT_FOUND: u32 = 101;
pub const TXERR_CODE_DECODE_TX: u32 = 102;

/// The application node implements the abci application trait and provides
/// functionality to execute services and manage storage.
pub struct Node {
    db: Arc<dyn Database>,
    app_state: AppState,
    services: HashMap<String, Box<dyn Service>>,
    commit_patches: Vec<Patch>,
    validate_tx_handler: Option<ValidateTxHandler>,
}

impl Node {
    // Create the app. This is called automatically when using the builder
    pub fn new(config: AppBuilder) -> Self {
        let db = config.db;

        let mut service_map: HashMap<String, Box<dyn Service>> = HashMap::new();
        for s in config.services {
            let route = s.route();
            // First come, first serve...
            if !service_map.contains_key(&route) {
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
                    TXERR_CODE_SIGNED_TX,
                    format!("Err parsing SignedTransaction: {:?}", &e),
                )
            }
        };

        // Return err if there are no services matching the route
        if !self.services.contains_key(&tx.route) {
            return TxResult::error(
                TXERR_CODE_SERVICE_NOT_FOUND,
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
        let service = self.services.get(&tx.route).unwrap();
        let result = match service.decode_tx(tx.msgid, tx.payload) {
            Ok(handler) => handler.execute(tx.sender, &fork),
            Err(e) => {
                return TxResult::error(
                    TXERR_CODE_DECODE_TX,
                    format!("Err decoding transaction: {}", e),
                )
            }
        };

        if result.code == 0 {
            // We only save patches from successful transactions
            self.commit_patches.push(fork.into_patch());
        }
        result
    }
}

// TOTAL HACK RIGHT NOW!
// Parse request_query path into 2 parts: route, path.  Route should
// point to the (route) name for the service.  Path is application specfic
// and can be used to determine how to handle a specific request.
fn parse_query_path(req_path: &String) -> (String, String) {
    let paths: Vec<&str> = req_path.split(REQ_QUERY_PATH_SEPERATOR).collect();
    // TODO: Better checking/approach needed here...
    (paths[0].into(), paths[1].into())
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

    // TODO:
    fn init_chain(&mut self, _req: &RequestInitChain) -> ResponseInitChain {
        // Add commit patches
        // Should add validators to storage
        ResponseInitChain::new()
    }

    fn query(&mut self, req: &RequestQuery) -> ResponseQuery {
        // parse the path, splitting on '**'
        let (route, query_path) = parse_query_path(&req.path);

        // Check if a service exists for this route
        if !self.services.contains_key(&route) {
            let mut failresp = ResponseQuery::new();
            failresp.code = 1u32;
            failresp.log = format!("cannot find query service for {}", route);
            return failresp;
        }

        // Update 'key' should just be a vec
        let key = req.data.clone();
        /*
        // Decode the request key
        let decoded_key = base64::decode(&req.data);
        if decoded_key.is_err() {
            let mut failresp = ResponseQuery::new();
            failresp.code = 1u32;
            failresp.log = format!(
                "cannot decode key for service {}.  It should be base64 encoded",
                route
            );
            return failresp;
        }
        */

        // Call the service
        let snapshot = self.db.snapshot();
        let result = self
            .services
            .get(&route)
            .unwrap()
            .query(query_path, key, &snapshot);

        // Return the result
        let mut response = ResponseQuery::new();
        response.code = result.code;
        //response.value = base64::encode(&result.value).to_bytes();
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
        // Commit accumulated patches to storage and clear commit_patches vec.
        for patch in self.commit_patches.drain(..) {
            self.db.merge(patch).unwrap();
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
        self.db.merge(fork.into_patch()).unwrap();

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
