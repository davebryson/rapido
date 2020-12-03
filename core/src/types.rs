/// Core types used by the framework
use std::cell::RefCell;

use abci::{Event, Pair};
use anyhow::{anyhow, Result};
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{Hash, PublicKey, SecretKey, Signature};
use protobuf::RepeatedField;

use crate::store::StoreView;

pub type AccountId = Vec<u8>;

/// Context is passed to handlers from the framework automatically.
/// It wraps information that can be used to process transactions
/// such as the sender of the tx, the encoded msg to process and
/// the ability to record events for Tendermint.
pub struct Context {
    /// The sender of the transaction (AccountId)
    pub sender: AccountId,
    /// The encoded message to process.
    pub msg: Vec<u8>,
    //event_manager: RefCell<EventManager>,
    events: RefCell<Vec<Event>>,
    appname: String,
}

impl Context {
    /// Create automatically by the framework for each incoming tx.
    pub fn new(tx: &SignedTransaction) -> Self {
        Self {
            sender: tx.sender(),
            msg: tx.msg(),
            //event_manager: RefCell::new(EventManager::new(tx.appname().into())),
            events: RefCell::new(Vec::new()),
            appname: tx.appname().into(),
        }
    }

    /// get the tx sender
    pub fn sender(&self) -> AccountId {
        /// Hmmm... this is ugly
        self.sender.clone()
    }

    /// Helper to decode a specific application msg.
    /// For example, if the app has a message such as:
    /// ```ignore
    /// #[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize)]
    ///  pub enum Msgs {
    ///     CreatePerson(String, u8),
    ///     IncPersonAge(String),
    ///  }
    /// ```
    /// You can decode it like this:
    /// ```ignore
    /// let m: Msgs = ctc.decode_msg().unwrap();
    /// ```
    pub fn decode_msg<M: BorshDeserialize + BorshSerialize>(
        &self,
    ) -> anyhow::Result<M, anyhow::Error> {
        M::try_from_slice(&self.msg).map_err(anyhow::Error::msg)
    }

    /// Dispatch an event that can be queried from Tendermint
    /// Example:
    /// ```ignore
    /// let pairs = &[("name", "bob"), ("employer", "Acme")];
    /// ctx.dispatch_event(pairs);
    ///```
    pub fn dispatch_event<T: Into<String>>(&self, event_type: T, pairs: &[(&str, &str)]) {
        let mut rf = RepeatedField::<Pair>::new();
        for (k, v) in pairs {
            let mut p = Pair::new();
            p.set_key(k.as_bytes().to_vec());
            p.set_value(v.as_bytes().to_vec());
            rf.push(p);
        }

        // Create a type with the appname: 'hello.transfer'
        let full_event_type = format!("{}.{}", self.appname, event_type.into());
        let mut e = Event::new();
        e.set_field_type(full_event_type.into());
        e.set_attributes(rf);
        self.events.borrow_mut().push(e);

        //self.event_manager
        //    .borrow_mut()
        //    .dispatch_event(event_type, pairs)
    }

    /// Return recorded events - called internally
    pub fn get_events(&self) -> RepeatedField<Event> {
        RepeatedField::from_vec(self.events.borrow().clone())
        //self.event_manager.borrow().get_events()
    }
}

/// Implement to create an authenticator for the app.  See `AppBuilder`.
/// A default Authenticator is used if one is not set by your application.
/// The default authenticator does not check txs or increment the nonce.
pub trait Authenticator: Sync + Send + 'static {
    /// Validate an incoming transaction to determine whether is should be included
    /// in the Tendermint tx mempool. Validation checks should be limited to
    /// checking signatures and other read-only operations against the store.
    /// Data read from the store is based on committed (not-cached) data.
    fn validate(
        &self,
        tx: &SignedTransaction,
        view: &StoreView,
    ) -> anyhow::Result<(), anyhow::Error>;

    /// Provide the logic to increment a nonce. This is usually needed for
    /// account based accounts to ensure the proper order of transactions.
    /// For example, if the same user sends multiple txs within the same block.
    /// This is called automatically in both check_tx, and deliver_tx.
    fn increment_nonce(
        &self,
        _tx: &SignedTransaction,
        _view: &mut StoreView,
    ) -> anyhow::Result<(), anyhow::Error> {
        Ok(())
    }
}

// Convert an AppModule in Box<App>
impl<T> From<T> for Box<dyn Authenticator>
where
    T: Authenticator,
{
    fn from(factory: T) -> Self {
        Box::new(factory) as Self
    }
}

/// Main trait to implement the core logic of your application.
pub trait AppModule: Sync + Send + 'static {
    /// This should return a application wide unique name for your application.
    /// The name used here will be used as the name of the `app` in a SignedTransaction
    /// And used to route txs to your AppModule.
    fn name(&self) -> String;

    /// Called on the initial start-up of the application. Can be used to establish
    /// initial state of your application. The data processed here should be passed
    /// through your AppModule implementation during AppBuilder setup.  For example,
    /// if you wanted to insert a list of tuples into the store during initialization
    /// of the application, you can do so like this:
    /// ```ignore
    /// let data = vec![(name, value), ...];
    /// AppBuilder.with_app(MyModule::new(data));
    /// ```
    /// How the data is processed below is up to the implementor.  
    fn initialize(&self, _view: &mut StoreView) -> Result<(), anyhow::Error> {
        Ok(())
    }

    /// Called to process a transaction. This is where your core logic goes.
    fn handle_tx(&self, ctx: &Context, view: &mut StoreView) -> Result<(), anyhow::Error>;

    /// Handle incoming queries to your application given a path and key.
    /// Queries are routed as `appname/{path}/...` where `appname` is from `name()`
    /// above.  Note: `appname` is NOT included in the path below. It's stripped
    /// when looking up the AppModule for the given query.  For example,
    /// if `name()` returns `mymodule` you can match on paths below after `mymodule` such
    /// as `/`
    ///    `/hello`
    ///    `/hello/world`
    /// However the *client* must call: `appmodule/hello`
    fn handle_query(
        &self,
        path: &str,
        key: Vec<u8>,
        view: &StoreView,
    ) -> Result<Vec<u8>, anyhow::Error>;
}

// Convert an AppModule in Box<App>
impl<T> From<T> for Box<dyn AppModule>
where
    T: AppModule,
{
    fn from(factory: T) -> Self {
        Box::new(factory) as Self
    }
}

/// SignedTransaction is used to transport transactions from the client to the your
/// application. It provides a wrapper around application specific information.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct SignedTransaction {
    // The sender/signer of the transaction
    sender: AccountId,
    // The name of the app to call. Same as `AppModule.name()`
    app: String,
    // The encoded bits of the enclosed message
    msg: Vec<u8>,
    // nonce
    nonce: u64,
    // the signature over the transaction
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

    /// Return the value of app
    pub fn appname(&self) -> &str {
        &*self.app
    }

    /// Return the Sender
    pub fn sender(&self) -> AccountId {
        self.sender.clone()
    }

    /// Return the nonce
    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    /// Get the signature
    pub fn signature(&self) -> Vec<u8> {
        self.signature.clone()
    }

    /// Get the encoded message
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
        // Hash order: sender, appname, msg, nonce
        let contents: Vec<u8> = vec![
            self.sender.clone(),
            self.app.as_bytes().to_vec(),
            self.msg.clone(),
            self.nonce().to_le_bytes().to_vec(),
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

/// Sign a transaction
pub fn sign_transaction(tx: &mut SignedTransaction, private_key: &SecretKey) {
    tx.signature = exonum_crypto::sign(&tx.hash()[..], private_key)
        .as_ref()
        .into();
}

/// Verify a transaction
pub fn verify_tx_signature(tx: &SignedTransaction, public_key: &PublicKey) -> bool {
    let hashed = tx.hash();
    match Signature::from_slice(&tx.signature[..]) {
        Some(signature) => exonum_crypto::verify(&signature, &hashed[..], public_key),
        None => false,
    }
}

mod tests {
    use super::*;

    #[derive(BorshDeserialize, BorshSerialize, PartialEq, Debug)]
    enum Message {
        Add(u16),
        Send(String),
    }

    #[test]
    fn test_signed_tx() {
        let accountid = vec![1];
        let (pk, sk) = exonum_crypto::gen_keypair();
        let mut tx =
            SignedTransaction::create(accountid.clone(), "example", Message::Add(10u16), 1u64);
        tx.sign(&sk);
        let encoded = tx.encode();

        let back = SignedTransaction::decode(&encoded).unwrap();
        assert!(verify_tx_signature(&back, &pk));

        let ctx = back.into_context();
        assert_eq!(Message::Add(10u16), ctx.decode_msg().unwrap());
        assert_eq!(accountid, ctx.sender);
        assert_eq!("example", back.appname());
    }
}
