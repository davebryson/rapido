use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{Hash, PublicKey, SecretKey, Signature};
use exonum_merkledb::{Fork, Snapshot};

#[derive(Debug)]
pub struct Context<'a> {
    pub sender: Vec<u8>,
    pub msgid: u8,
    pub msg: Vec<u8>,
    pub fork: &'a Fork,
}

impl<'a> Context<'a> {
    pub fn from_tx(tx: SignedTransaction, fork: &'a Fork) -> Self {
        Self {
            sender: tx.sender.clone(),
            msgid: tx.msgid,
            msg: tx.msg.clone(),
            fork,
        }
    }
}

/// Function type for the abci checkTx handler.  This function should
/// contain the logic to determine whether to accept or reject transactions
/// from the Tendermint memory pool. Note: it only provides read-only
/// access to storage. Validation checks should be limited to
/// checking signatures or other read-only operations.
pub type AuthenticationHandler =
    fn(tx: &SignedTransaction, snapshot: &Box<dyn Snapshot>) -> Result<(), anyhow::Error>;

pub trait AppModule: Sync + Send {
    /// The routing name of the service. This cooresponds to the route field in a SignedTransaction.
    /// Your service should return a route name that's unique across all services.  Internally the
    /// Rapido node stores services keyed by the route on a first come basis on creation.
    fn name(&self) -> &'static str;

    /// Called on the initial start-up of the application. Can be used to establish
    /// initial state for your application. Provides a borrowed view of genesis data
    /// for each application to process as needed.
    // TODO: Add validator info, and chain_id
    fn initialize(&self, _fork: &Fork, _data: Option<&Vec<u8>>) -> Result<(), anyhow::Error> {
        Ok(())
    }

    // Dispatch a transaction to internal handlers
    fn handle_tx(&self, ctx: Context) -> Result<(), anyhow::Error>;

    // TODO: Improve this
    fn handle_query(
        &self,
        path: &str,
        key: Vec<u8>,
        snapshot: &Box<dyn Snapshot>,
    ) -> Result<Vec<u8>, anyhow::Error>;
}

/// SignedTransaction is used to transport transactions from the client to the your
/// application. It provides a wrapper around application specific transactions.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct SignedTransaction {
    /// The sender/signer of the transaction
    pub sender: Vec<u8>,
    /// The name of the app to call
    pub app: String,
    /// An ID to identify the transaction. This can be used to determine which msg to decode
    pub msgid: u8,
    /// The encoded bits of the enclosed message
    pub msg: Vec<u8>,
    /// the signature over the transaction
    pub signature: Vec<u8>,
}

impl SignedTransaction {
    /// Create a new SignedTransaction
    pub fn new<M>(sender: Vec<u8>, app: &'static str, msgid: u8, msg: M) -> Self
    where
        M: BorshSerialize + BorshDeserialize,
    {
        let payload = msg.try_to_vec().unwrap();
        Self {
            sender,
            app: String::from(app),
            msgid,
            msg: payload,
            signature: Default::default(),
        }
    }

    /// Convenience method to encode the transaction using BorshSerialization
    /// without having to import the associated trait.
    /// Will `panic` on a serialization error.
    pub fn encode(&self) -> Vec<u8> {
        self.try_to_vec().expect("encoding signed transaction")
    }

    fn hash(&self) -> Hash {
        // Hash order: sender, appname, msgid, msg
        let contents: Vec<u8> = vec![
            self.sender.clone(),
            self.app.as_bytes().to_vec(),
            vec![self.msgid],
            self.msg.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();
        exonum_crypto::hash(&contents[..])
    }
}

pub fn sign_transaction(tx: &mut SignedTransaction, private_key: &SecretKey) {
    tx.signature = exonum_crypto::sign(&tx.hash()[..], private_key)
        .as_ref()
        .into();
}

pub fn verify_tx_signature(tx: &SignedTransaction, public_key: &PublicKey) -> bool {
    let hashed = tx.hash();
    match Signature::from_slice(&tx.signature[..]) {
        Some(signature) => exonum_crypto::verify(&signature, &hashed[..], public_key),
        None => false,
    }
}
