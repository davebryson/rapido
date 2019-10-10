pub use self::tx::{verify_tx_signature, Transaction};

use abci::*;
use exonum_crypto::Hash;
use exonum_merkledb::{Fork, Snapshot};

pub(crate) mod schema;
mod tx;

/// User-friendly name for an Account Identifier
pub type AccountId = Vec<u8>;

/// Function type for the abci checkTx handler.  This function should
/// containt logic to determine whether to accept or reject transactions
/// from the Tendermint memory pool. Note: it only provides read-only
/// access to storage.  So validation checks should be limited to validated
/// Transaction messages and/or verifying sender signatures.
pub type ValidateTxHandler = fn(tx: &Transaction, snapshot: &Box<dyn Snapshot>) -> TxResult;

/// Implement this trait for your application logic.
/// Each application may have 1 or more of these. Each service is
/// keyed by the application by the 'route'. So you use a unique route name.
pub trait Service: Sync + Send {
    // The routing name of the service. This cooresponds to
    // the route field in a Tx.
    fn route(&self) -> String;

    // fn genesis(&self, validators, fork) -> TxResult;

    // Main entry point to your application. Here's where you
    // implement state transistion logic and interact with storage.
    fn execute(&self, tx: &Transaction, fork: &Fork) -> TxResult;

    // Main entry point for abci request queries. 'snapshot' provides
    // read-only access to storage.  You can use path to do your own routing
    // for internal handlers.  NOTE: For now, I use a bit of a hack to map
    // query requests to services.   Clients sending queries should use the
    // form 'routename**your_application_path' in RequestQuery.data.
    // Where 'routename' IS the service route name. The value '**' is used
    // internally as a seperator, and 'your_application_path' is specific to your application.
    // So, if request_query.data == 'routename**your_application_path', then:
    //   'routename' == the service route name
    //   'your_application_path' is the application specific path.
    fn query(&self, path: String, key: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> QueryResult;

    // This function is called on ABCI commit to accumulate a new
    // root hash across all services. You should return the current
    // root hash from you state store(s).  If your app uses more than one form
    // of storage, you should return an accumulates hash of all you storage root hashes.
    fn root_hash(&self, fork: &Fork) -> Hash;
}

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

#[cfg(test)]
mod tests;
