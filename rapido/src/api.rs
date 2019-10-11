use abci::{ResponseCheckTx, ResponseDeliverTx};
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{CryptoHash, Hash, PublicKey, SecretKey, Signature};
use exonum_merkledb::{Fork, Snapshot};
use failure::ensure;

/// Maximum length of an AccountId used in
/// the sender field of a signed transaction.
pub const ACCOUNT_ID_LENGTH: usize = 32;

/// A user-friendly type for [u8; 32] used to identify an account
pub type AccountId = [u8; ACCOUNT_ID_LENGTH];

/// Function type for the abci checkTx handler.  This function should
/// contain the logic to determine whether to accept or reject transactions
/// from the Tendermint memory pool. Note: it only provides read-only
/// access to storage.  So validation checks should be limited to checking signatures
pub type ValidateTxHandler = fn(tx: &SignedTransaction, snapshot: &Box<dyn Snapshot>) -> TxResult;

/// Implement this trait for your Service. Each service is
/// keyed by the application by the 'route'. So you use a unique route name.
pub trait Service: Sync + Send {
    // The routing name of the service. This cooresponds to
    // the route field in a Tx.
    fn route(&self) -> String;

    // TODO:
    // fn genesis(&self, validators, fork) -> TxResult;

    // Decode incoming transactions for the applications.  Each service may contain
    // 1 or more transactions the perform a state transistion. This should contain the
    // logic needed to select the application transaction to decode based on the
    // user-assigned 'msgid'.
    fn decode_tx(
        &self,
        msgid: u16,
        payload: Vec<u8>,
    ) -> Result<Box<dyn Transaction>, std::io::Error>;

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
    // of storage, you should return an accumulated hash of all your storage root hashes.
    // The result of this function becomes the tendermint 'app hash'.
    fn root_hash(&self, fork: &Fork) -> Hash;
}

/// TxResult is returned from Transactions & the validateTxHandler. They areautomatically
/// converted to the associated ResponseCheck-DeliverTx message. Any non-zero code indicates an error.
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

    // Returns and error code with the reason
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

/// Type returned from a service query handler.  QueryResults will be
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

/// Main trait to implement for your application transactions.  Execute is ran
/// during the abci 'deliver_tx' function.
pub trait Transaction: Send + Sync {
    fn execute(&self, sender: AccountId, fork: &Fork) -> TxResult;
}

/// SignedTransaction is used to transport transactions from the client to the your
/// application.  The provide a wrapper around application transactions.
/// Note: This will evolve to provide more flexibilty in the future...
#[derive(BorshSerialize, BorshDeserialize)]
pub struct SignedTransaction {
    pub sender: AccountId,
    pub route: String,
    pub msgid: u16,
    pub payload: Vec<u8>,
    pub signature: Vec<u8>,
}

impl SignedTransaction {
    // Create a new SignedTransaction
    pub fn new<R, M>(sender: AccountId, route: R, msgid: u16, msg: M) -> Self
    where
        R: Into<String>,
        M: BorshSerialize + BorshDeserialize + Transaction,
    {
        let payload = msg.try_to_vec().unwrap();
        Self {
            sender,
            route: route.into(),
            msgid,
            payload,
            signature: Default::default(),
        }
    }
}

impl CryptoHash for SignedTransaction {
    fn hash(&self) -> Hash {
        // Need to clean up this mess...
        let mut sender_bits = vec![0u8; 32];
        sender_bits.copy_from_slice(&self.sender[..].to_vec());
        let route_bits = self.route.as_bytes().to_vec();
        let mut msgid_bits = vec![0u8; 2];
        msgid_bits.copy_from_slice(&self.msgid.to_le_bytes());

        // Hash order: sender, route, msgid, payload
        let contents: Vec<u8> = vec![sender_bits, route_bits, msgid_bits, self.payload.clone()]
            .into_iter()
            .flatten()
            .collect();

        exonum_crypto::hash(&contents[..])
    }
}

// Verify the signature for a signed transaction
pub fn verify_tx_signature(tx: &SignedTransaction, public_key: &PublicKey) -> bool {
    let hashed = tx.hash();
    match Signature::from_slice(&tx.signature[..]) {
        Some(signature) => exonum_crypto::verify(&signature, &hashed[..], public_key),
        None => false,
    }
}

// Sign a transaction
pub fn sign_transaction(
    tx: &mut SignedTransaction,
    private_key: &SecretKey,
) -> Result<(), failure::Error> {
    ensure!(tx.sender.len() == ACCOUNT_ID_LENGTH, "AccountId is empty");
    tx.signature = exonum_crypto::sign(&tx.hash()[..], private_key)
        .as_ref()
        .into();
    Ok(())
}
