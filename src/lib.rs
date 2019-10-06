///!
///! Rapido is a simple framework for creating Tendermint applications.
///! It uses Protobuf3 for the message format and the exonum_merkle_db
///! for state storage. To create an app you need to:
///!  - Define your storage schema.
///!  - Create your associated Services.
///!  - Define a handler to validate incoming transactions
///!  - Assemble the application with the builder
///!  - And finally, run it with abci.
///!
pub use self::tx::Tx;

mod state_schema;
mod tx;

use abci::*;
use exonum_crypto::Hash;
use exonum_merkledb::{BinaryValue, Database, Fork, Patch, Snapshot};
use protobuf::error::{ProtobufError, ProtobufResult};
use protobuf::Message;
use std::collections::HashMap;
use std::sync::Arc;

use state_schema::{AppState, AppStateSchema};

const NAME: &str = "rapido_v1";
const REQ_QUERY_PATH_SEPERATOR: &str = "**";

/// Function type for the abci checkTx handler.  This function should
/// containt logic to determine whether to accept or reject transactions
/// from the Tendermint memory pool. Note: it only provides read-only
/// access to storage.  So validation checks should be limited to validated
/// Transaction messages and/or verifying sender signatures.
pub type ValidateTxHandler = fn(tx: &Tx, snapshot: &Box<dyn Snapshot>) -> TxResult;

/// Implement this trait for your application logic.
/// Each application may have 1 or more of these. Each service is
/// keyed by the application by the 'route'. So you use a unique route name.
pub trait Service {
    // The routing name of the service. This cooresponds to
    // the route field in a Tx.
    fn route(&self) -> String;

    // Main entry point to your application. Here's where you
    // implement state transistion logic and interact with storage.
    fn execute(&self, tx: &Tx, fork: &Fork) -> TxResult;

    // Main entry point for abci request queries. 'snapshot' provides
    // read-only access to storage.  You can use path to do your own routing
    // for internal handlers.  NOTE: For now, I use a bit of a hack to map
    // query requests to services.   Clients sending queries should use the
    // form 'routename**your_application_path' in RequestQuery.data.
    // Where 'routename' IS the service route name. The value '**' is used
    // internally as a seperator, and 'your_application_path' is specific to your application.
    // So, if request_query.data == 'routename**your_application_path', then:
    //   routename == the service route name
    //   your_application_path is the application specific path.
    fn query(&self, path: String, key: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> QueryResult;

    // This function is called on ABCI commit to accumulate a new
    // root hash across all services. You should return the current
    // root hash from you state store(s).  If your app uses more than one form
    // of storage, you should return an accumulates hash of all you storage root hashes.
    fn root_hash(&self, fork: &Fork) -> Hash;
}

/// TODO: Convert this to a proper Result !
/// TxResult is returned from service/validateTxHandler and automatically converted to the
/// associated ResponseCheck-DeliverTx message. Any non-zero code indicates an error.
/// Applications are responsible for creating their own meaningful codes and messages (log).
///   
/// Why not use a std::Result for this? In the future we need to add support for events to
/// this structure.
pub struct TxResult {
    pub code: u32,
    pub log: String,
}

impl TxResult {
    // Construct a new code and log/message
    pub fn new<T: Into<String>>(code: u32, log: T) -> Self {
        Self {
            code,
            log: log.into(),
        }
    }

    // Returns a 0 (ok) code with and empty log message
    pub fn ok() -> Self {
        Self {
            code: 0,
            log: "".to_string(),
        }
    }

    // Returns and error with the reason
    pub fn error<T: Into<String>>(code: u32, reason: T) -> Self {
        Self {
            code,
            log: reason.into(),
        }
    }
}

/// Type returned from service query handlers.  QueryResults will be
/// converted to proper abci types internally.
pub struct QueryResult {
    pub code: u32,
    pub value: Vec<u8>,
}

impl QueryResult {
    pub fn ok(value: Vec<u8>) -> Self {
        Self { code: 0, value }
    }

    pub fn error(code: u32) -> Self {
        Self {
            code,
            value: Vec::new(),
        }
    }
}

// Convert a TxResult into a abci.checkTx response
#[doc(hidden)]
impl Into<ResponseCheckTx> for TxResult {
    fn into(self) -> ResponseCheckTx {
        let mut resp = ResponseCheckTx::new();
        resp.set_code(self.code);
        resp.set_log(self.log);
        resp
    }
}

// Convert a TxResult into a abci.deliverTx response
#[doc(hidden)]
impl Into<ResponseDeliverTx> for TxResult {
    fn into(self) -> ResponseDeliverTx {
        let mut resp = ResponseDeliverTx::new();
        resp.set_code(self.code);
        resp.set_log(self.log);
        resp
    }
}

pub trait IntoProtoBytes<P> {
    // Encode a Rust struct to Protobuf bytes.
    fn into_proto_bytes(self) -> ProtobufResult<Vec<u8>>;
}

pub trait FromProtoBytes<P>: Sized {
    // Decode a Rust struct from encoded Protobuf bytes.
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ProtobufError>;
}

impl IntoProtoBytes<Tx> for Tx {
    fn into_proto_bytes(self) -> ProtobufResult<Vec<u8>> {
        self.write_to_bytes()
    }
}

impl FromProtoBytes<Tx> for Tx {
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ProtobufError> {
        protobuf::parse_from_bytes::<Self>(bytes)
    }
}

/// Builder to assemble an application
pub struct AppBuilder {
    pub db: Arc<dyn Database>,
    pub handlers: Vec<Box<dyn Service>>,
    pub validate_tx_handler: Option<ValidateTxHandler>,
}

impl AppBuilder {
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self {
            db,
            handlers: Vec::new(),
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
        self.handlers.push(handler);
        self
    }

    // Call to return a configured node. This consumes the underlying builder
    pub fn finish(self) -> Node {
        Node::new(self)
    }
}

/// The application node.  Implements the abci application trait and provides
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

        let mut map: HashMap<String, Box<dyn Service>> = HashMap::new();
        for h in config.handlers {
            let route = h.route();
            // Should check it doesn't already exist !
            map.insert(route, h);
        }

        Self {
            db: db.clone(),
            app_state: AppState::default(),
            services: map,
            commit_patches: Vec::new(),
            validate_tx_handler: config.validate_tx_handler,
        }
    }

    fn run_tx(&mut self, is_check: bool, raw_tx: Vec<u8>) -> TxResult {
        let tx = match Tx::from_proto_bytes(&raw_tx[..]) {
            Ok(tx) => tx,
            Err(e) => return TxResult::error(11, format!("Err parsing Tx: {:?}", &e)),
        };

        // Return err if there are no handlers matching the route
        if !self.services.contains_key(&tx.route) {
            return TxResult::error(12, format!("Handler not found for route: {}", tx.route));
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
        let result = service.execute(&tx, &fork);
        if result.code == 0 {
            // We only save patches from successful txs
            self.commit_patches.push(fork.into_patch());
        }
        result
    }
}

// Parse request_query path into 2 parts: route, path.  Route should
// point to the (route) name for the service.  Path is application specfic
// and can be used to determine how to handle a specific request.
fn parse_query_path(req_path: &String) -> (String, String) {
    let paths: Vec<&str> = req_path.split(REQ_QUERY_PATH_SEPERATOR).collect();
    (paths[0].into(), paths[1].into())
}

#[doc(hidden)]
impl abci::Application for Node {
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

    fn init_chain(&mut self, _req: &RequestInitChain) -> ResponseInitChain {
        // Add commit patches
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

        // Call the service
        let snapshot = self.db.snapshot();
        let result =
            self.services
                .get(&route)
                .unwrap()
                .query(query_path, decoded_key.unwrap(), &snapshot);

        // Return the result
        let mut response = ResponseQuery::new();
        response.code = result.code;
        response.value = base64::encode(&result.value).to_bytes();
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
        ResponseEndBlock::new()
    }

    fn commit(&mut self, _req: &RequestCommit) -> ResponseCommit {
        // Commit all patches to storage, clearing commit_patches
        for patch in self.commit_patches.drain(..) {
            self.db.merge(patch).unwrap();
        }

        let fork = self.db.fork();
        // Calculate new root hash from all services
        let mut hashes: Vec<Hash> = Vec::new();
        for (_, service) in &self.services {
            hashes.push(service.root_hash(&fork));
        }
        let state_root = exonum_merkledb::root_hash(&hashes);

        // Update app state
        self.app_state.hash = state_root.to_bytes();
        self.app_state.version = self.app_state.version + 1;

        // Commit app state
        let commit_schema = AppStateSchema::new(&fork);
        commit_schema.app_state().set(AppState {
            version: self.app_state.version,
            hash: self.app_state.hash.clone(),
        });

        // Merge new commit info to db
        self.db.merge(fork.into_patch()).unwrap();

        let mut resp = ResponseCommit::new();
        resp.set_data(self.app_state.hash.clone());
        resp
    }
}
