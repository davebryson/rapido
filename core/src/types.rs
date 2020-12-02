use std::cell::RefCell;

use abci::{Event, Pair};
use anyhow::{anyhow, Result};
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{Hash, PublicKey, SecretKey, Signature};
use protobuf::RepeatedField;

use crate::store::StoreView;

pub type AccountId = String;

pub struct EventManager {
    pub appname: String,
    events: Vec<Event>,
}

impl EventManager {
    pub fn new(appname: String) -> Self {
        Self {
            appname: appname,
            events: Vec::new(),
        }
    }

    /// Example:
    /// let pairs = &[("name", "bob"), ("employer", "Acme")];
    /// eventmanager.emit_event(pairs);
    pub fn dispatch_event(&mut self, event_type: &str, pairs: &[(&str, &str)]) {
        let mut rf = RepeatedField::<Pair>::new();
        for (k, v) in pairs {
            let mut p = Pair::new();
            p.set_key(k.as_bytes().to_vec());
            p.set_value(v.as_bytes().to_vec());
            rf.push(p);
        }

        // Create a type with the appname: 'hello.transfer'
        let full_event_type = format!("{}.{}", self.appname, event_type);
        let mut e = Event::new();
        e.set_field_type(full_event_type.into());
        e.set_attributes(rf);
        self.events.push(e);
    }

    pub fn get_events(&self) -> RepeatedField<Event> {
        RepeatedField::from_vec(self.events.clone())
    }
}

pub struct Context {
    pub sender: AccountId,
    pub msg: Vec<u8>,
    event_manager: RefCell<EventManager>,
}

impl Context {
    pub fn new(tx: &SignedTransaction) -> Self {
        Self {
            sender: tx.sender(),
            msg: tx.msg(),
            event_manager: RefCell::new(EventManager::new(tx.appname().into())),
        }
    }

    /// Decode a msg in the transaction
    pub fn decode_msg<M: BorshDeserialize + BorshSerialize>(&self) -> M {
        M::try_from_slice(&self.msg).expect("decode")
    }

    pub fn dispatch_event(&self, event_type: &str, pairs: &[(&str, &str)]) {
        self.event_manager
            .borrow_mut()
            .dispatch_event(event_type, pairs)
    }

    pub fn get_events(&self) -> RepeatedField<Event> {
        self.event_manager.borrow().get_events()
    }
}

/// Function type for the abci checkTx handler.  This function should
/// contain the logic to determine whether to accept or reject transactions
/// from the Tendermint memory pool. Note: it only provides read-only
/// access to storage. Validation checks should be limited to
/// checking signatures or other read-only operations.
//pub type AuthenticationHandler =
//    fn(tx: &SignedTransaction, view: &mut StoreView) -> Result<(), anyhow::Error>;

/// Implement this type for the abci checkTx handler.  This function should
/// contain the logic to determine whether to accept or reject transactions
/// from the Tendermint memory pool. Note: it only provides read-only
/// access to storage. Validation checks should be limited to
/// checking signatures or other read-only operations.
pub trait Authenticator: Sync + Send + 'static {
    /// Validate an incoming transaction to determine whether is should be included
    /// in the tx mempool.
    fn validate(
        &self,
        tx: &SignedTransaction,
        view: &StoreView,
    ) -> anyhow::Result<(), anyhow::Error>;

    /// Implement this function to increment the nonce of the caller.
    fn increment_nonce(
        &self,
        _tx: &SignedTransaction,
        _view: &mut StoreView,
    ) -> anyhow::Result<(), anyhow::Error> {
        Ok(())
    }
}

pub trait AppModule: Sync + Send + 'static {
    /// The routing name of the service. This cooresponds to the route field in a SignedTransaction.
    /// Your service should return a route name that's unique across all services.  Internally the
    /// Rapido node stores services keyed by the route on a first come basis on creation.
    fn name(&self) -> &'static str;

    /// Called on the initial start-up of the application. Can be used to establish
    /// initial state for your application. Provides a borrowed view of genesis data
    /// for each application to process as needed.
    // TODO: Add validator info, and chain_id
    fn initialize(&self, _view: &mut StoreView) -> Result<(), anyhow::Error> {
        Ok(())
    }

    // Dispatch a transaction to internal handlers
    fn handle_tx(&self, ctx: &Context, view: &mut StoreView) -> Result<(), anyhow::Error>;

    // Hand a query for a given subpath.
    fn handle_query(
        &self,
        path: &str,
        key: Vec<u8>,
        view: &StoreView,
    ) -> Result<Vec<u8>, anyhow::Error>;
}

impl<T> From<T> for Box<dyn AppModule>
where
    T: AppModule,
{
    fn from(factory: T) -> Self {
        Box::new(factory) as Self
    }
}

/// SignedTransaction is used to transport transactions from the client to the your
/// application. It provides a wrapper around application specific transactions.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct SignedTransaction {
    /// The id of the sender/signer of the transaction
    sender: AccountId,
    /// The name of the app to call
    app: String,
    /// The encoded bits of the enclosed message
    msg: Vec<u8>,
    // nonce
    nonce: u64,
    /// the signature over the transaction
    signature: Vec<u8>,
}

impl SignedTransaction {
    /// Create a new SignedTransaction
    pub fn create<S: Into<AccountId>, M>(sender: S, app: &'static str, msg: M, nonce: u64) -> Self
    where
        M: BorshSerialize + BorshDeserialize,
    {
        let payload = msg.try_to_vec().unwrap();
        Self {
            sender: sender.into(),
            app: String::from(app),
            msg: payload,
            nonce,
            signature: Default::default(),
        }
    }

    pub fn appname(&self) -> &str {
        &*self.app
    }

    pub fn sender(&self) -> String {
        self.sender.clone()
    }

    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    pub fn signature(&self) -> Vec<u8> {
        self.signature.clone()
    }

    pub fn msg(&self) -> Vec<u8> {
        self.msg.clone()
    }

    /// Convenience method to encode the transaction using BorshSerialization
    /// without having to import the associated trait.
    /// Will `panic` on a serialization error.
    pub fn encode(&self) -> Vec<u8> {
        self.try_to_vec().expect("encoding signed transaction")
    }

    /// Decode
    pub fn decode(raw: &[u8]) -> anyhow::Result<Self, anyhow::Error> {
        SignedTransaction::try_from_slice(raw)
            .map_err(|_| anyhow!("problem decoding the signed tx"))
    }

    /// Sign the transaction
    pub fn sign(&mut self, private_key: &SecretKey) {
        self.signature = exonum_crypto::sign(&self.hash()[..], private_key)
            .as_ref()
            .into();
    }

    fn hash(&self) -> Hash {
        // Hash order: sender, appname, msgid, msg
        let contents: Vec<u8> = vec![
            self.sender.as_bytes().to_vec(),
            self.app.as_bytes().to_vec(),
            self.msg.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();
        exonum_crypto::hash(&contents[..])
    }

    /// Convert the tx to a context
    pub fn into_context(&self) -> Context {
        Context::new(self)
    }

    // Encode the Tx as a hex value (prefixed with 0x).
    // Can be used to send Txs via http GET api.
    pub fn to_hex(&self) -> String {
        format!("0x{:}", hex::encode(self.encode()))
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

mod tests {
    use super::*;

    #[test]
    fn test_tx() {
        #[derive(BorshDeserialize, BorshSerialize, PartialEq, Debug)]
        enum Message {
            Add(u16),
            Send(String),
        }

        let accountid = "dave";
        let (pk, sk) = exonum_crypto::gen_keypair();
        let mut tx =
            SignedTransaction::create(accountid.clone(), "example", Message::Add(10u16), 1u64);
        tx.sign(&sk);
        let encoded = tx.encode();

        let back = SignedTransaction::decode(&encoded).unwrap();
        assert!(verify_tx_signature(&back, &pk));

        let ctx = back.into_context();
        assert_eq!(Message::Add(10u16), ctx.decode_msg());
        assert_eq!(accountid, ctx.sender);
        assert_eq!("example", back.appname());
    }
}
