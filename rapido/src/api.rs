use abci::{ResponseCheckTx, ResponseDeliverTx};
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{CryptoHash, Hash, PublicKey, SecretKey, Signature};
use exonum_merkledb::{Fork, Snapshot};

use crate::account_address::AccountAddress;

/// Function type for the abci checkTx handler.  This function should
/// contain the logic to determine whether to accept or reject transactions
/// from the Tendermint memory pool. Note: it only provides read-only
/// access to storage. Validation checks should be limited to
/// checking signatures or other read-only operations.
pub type ValidateTxHandler = fn(tx: &SignedTransaction, snapshot: &Box<dyn Snapshot>) -> TxResult;

/// Service is the starting point for your application. Each service may operate
/// on 1 or transactions. Services are keyed internally by 'route'.
pub trait Service: Sync + Send {
    /// The routing name of the service. This cooresponds to the route field in a SignedTransaction.
    /// Your service should return a route name that's unique across all services.  Internally the
    /// Rapido node stores services keyed by the route on a first come basis on creation.
    fn route(&self) -> String;

    /// Called on the initial start-up of the application. Can be used to establish
    /// initial state for your application.
    // TODO: Add validator info, and chain_id
    fn genesis(&self, _fork: &Fork) -> TxResult {
        TxResult::ok()
    }

    /// Decode incoming transactions for the application.  Each service may contain
    /// 1 or more transactions that perform a state transistion. This function should
    /// contain the logic to select the application transaction to decode based on the
    /// user-assigned 'txid'.
    fn decode_tx(&self, txid: u8, payload: Vec<u8>)
        -> Result<Box<dyn Transaction>, std::io::Error>;

    /// Main entry point for abci request queries. 'snapshot' provides
    /// read-only access to storage.  You can use path to do your own routing
    /// for internal handlers. `path` below is extracted from the AbciQuery.path.
    /// Proper queries should be in the form: 'routename/path', where 'routename'
    /// is the name of the service, and 'path' is used to route to a specific application query.
    fn query(&self, path: String, key: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> QueryResult;

    /// This function is called on ABCI commit to accumulate a new
    /// root hash across all services. You should return the current
    /// root hash from you state store(s).  If your app uses more than one form
    /// of storage, you should return an accumulated hash of all your storage root hashes.
    /// The result of this function becomes the tendermint 'app hash'.
    fn root_hash(&self, fork: &Fork) -> Hash;
}

/// TxResult is returned from Transactions & the validateTxHandler and are automatically
/// converted to the associated ResponseCheck/DeliverTx. Any non-zero code indicates an error.
/// Applications are responsible for creating their own meaningful codes and messages (log).
///   
/// Why not use a std::Result for this? In the future we need to add support for events to
/// this structure.
pub struct TxResult {
    pub code: u32,
    pub log: String,
}

impl TxResult {
    /// Construct a new code and log/message
    pub fn new<T: Into<String>>(code: u32, log: T) -> Self {
        Self {
            code,
            log: log.into(),
        }
    }

    /// Returns a 0 (ok) code with and empty log message
    pub fn ok() -> Self {
        Self {
            code: 0,
            log: "".to_string(),
        }
    }

    /// Returns and error code with the reason
    pub fn error<T: Into<String>>(code: u32, reason: T) -> Self {
        Self {
            code,
            log: reason.into(),
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

/// Returned from a service query handler to indicate success or failure.
/// `QueryResult::ok(data)` is a successful query with the resulting `data`
/// QueryResults will be converted to proper abci types internally.
/// TODO: Expand to include proof.
pub struct QueryResult {
    pub code: u32,
    pub value: Vec<u8>,
}

impl QueryResult {
    /// Ok: `value` is the result to return from running the query. Since `value`
    /// is a byte array, it's the responsibly of the caller (client) to decode it.
    pub fn ok(value: Vec<u8>) -> Self {
        Self { code: 0, value }
    }

    /// Error: provide an application error code
    pub fn error(code: u32) -> Self {
        Self {
            code,
            value: Vec::new(),
        }
    }
}

/// Transaction is the heart of your application logic. `execute()` is ran
/// during the abci 'deliver_tx' function and should contain the state transition
/// logic. Each Service may support 1 or more transactions.
pub trait Transaction: Send + Sync {
    /// Execute the logic associated with this transaction. Implement your
    /// business logic here. `Fork` provides mutable access to the associated state store.
    /// AccountAddress is provided by the SignedTransaction used to transport this tx.
    fn execute(&self, sender: AccountAddress, fork: &Fork) -> TxResult;
}

/// SignedTransaction is used to transport transactions from the client to the your
/// application. It provides a wrapper around application specific transactions.
/// Note: This will evolve to provide more flexibilty in the future...
#[derive(BorshSerialize, BorshDeserialize)]
pub struct SignedTransaction {
    /// The sender/signer of the transaction
    pub sender: AccountAddress,
    /// The unique route to the Service
    pub route: String,
    /// An ID to identify the transaction. This can be used to determine which Transaction
    /// to decode in `service.decode_tx`
    pub txid: u8,
    /// The encoded bits of the enclosed transaction
    pub payload: Vec<u8>,
    /// the signature over the transaction
    pub signature: Vec<u8>,
}

impl SignedTransaction {
    /// Create a new SignedTransaction
    pub fn new<R, M>(sender: AccountAddress, route: R, txid: u8, msg: M) -> Self
    where
        R: Into<String>,
        M: BorshSerialize + BorshDeserialize + Transaction,
    {
        let payload = msg.try_to_vec().unwrap();
        Self {
            sender,
            route: route.into(),
            txid,
            payload,
            signature: Default::default(),
        }
    }
}

impl CryptoHash for SignedTransaction {
    // Hash the contents for signing
    fn hash(&self) -> Hash {
        // Hash order: sender, route, txid, payload
        let contents: Vec<u8> = vec![
            self.sender.to_vec(),
            self.route.as_bytes().to_vec(),
            vec![self.txid],
            self.payload.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();
        exonum_crypto::hash(&contents[..])
    }
}

/// Verify the signature for a signed transaction
pub fn verify_tx_signature(tx: &SignedTransaction, public_key: &PublicKey) -> bool {
    let hashed = tx.hash();
    match Signature::from_slice(&tx.signature[..]) {
        Some(signature) => exonum_crypto::verify(&signature, &hashed[..], public_key),
        None => false,
    }
}

/// Sign a transaction
pub fn sign_transaction(tx: &mut SignedTransaction, private_key: &SecretKey) {
    tx.signature = exonum_crypto::sign(&tx.hash()[..], private_key)
        .as_ref()
        .into();
}
