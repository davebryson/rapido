use abci::{Event, Pair};
use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{Hash, PublicKey, SecretKey, Signature};
use exonum_merkledb::{Fork, Snapshot};
use protobuf::RepeatedField;
use std::cell::{Ref, RefCell, RefMut};

use crate::store::CacheMap;

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
    pub sender: Vec<u8>,
    pub msgid: u8,
    pub msg: Vec<u8>,
    event_manager: RefCell<EventManager>,
}

impl Context {
    pub fn new(tx: &SignedTransaction) -> Self {
        Self {
            sender: tx.sender.clone(),
            msgid: tx.msgid,
            msg: tx.msg.clone(),
            event_manager: RefCell::new(EventManager::new(tx.app.clone())),
        }
    }

    /*
    pub fn from_tx(tx: SignedTransaction, fork: &'a Fork) -> Self {
        Self {
            sender: tx.sender.clone(),
            msgid: tx.msgid,
            msg: tx.msg.clone(),
            fork,
            event_manager: RefCell::new(EventManager::new(tx.app.clone())),
        }
    }*/

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
pub type AuthenticationHandler =
    fn(tx: &SignedTransaction, store: &mut CacheMap) -> Result<(), anyhow::Error>;

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
    fn handle_tx(&self, ctx: &Context, store: &mut CacheMap) -> Result<(), anyhow::Error>;

    // Hand a query for a given subpath.
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
    // nonce
    pub nonce: u64,
    /// the signature over the transaction
    pub signature: Vec<u8>,
}

impl SignedTransaction {
    /// Create a new SignedTransaction
    pub fn new<M>(sender: Vec<u8>, app: &'static str, msgid: u8, msg: M, nonce: u64) -> Self
    where
        M: BorshSerialize + BorshDeserialize,
    {
        let payload = msg.try_to_vec().unwrap();
        Self {
            sender,
            app: String::from(app),
            msgid,
            msg: payload,
            nonce,
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
