use anyhow::bail;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::PublicKey;

use rapido_core::{
    verify_tx_signature, AccountId, AppModule, Authenticator, Context, SignedTransaction, Store,
    StoreView,
};

// Model
#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub struct Model {
    pub value: u8,
}

impl Model {
    pub fn decode(raw: Vec<u8>) -> Self {
        Self::try_from_slice(&raw[..]).expect("decode model")
    }
}

// Make the model tree compliant
impl_store_values!(Model);

// Store
pub struct ModelStore {
    name: String,
}

impl ModelStore {
    pub fn load(appname: &str) -> Self {
        Self {
            name: format!("{:}.store", appname),
        }
    }
}

impl Store for ModelStore {
    type Key = AccountId;
    type Value = Model;

    fn name(&self) -> String {
        self.name.clone()
    }
}

// Messages
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum Msgs {
    Create(u8),
    Inc,
}

// App logic
pub struct ModelApp {
    name: String,
}

impl ModelApp {
    pub fn new(name: &str) -> Self {
        Self { name: name.into() }
    }
}

impl AppModule for ModelApp {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn handle_tx(&self, ctx: &Context, cache: &mut StoreView) -> Result<(), anyhow::Error> {
        let msg: Msgs = ctx.decode_msg()?;
        match msg {
            Msgs::Create(val) => {
                let store = ModelStore::load(&self.name);
                store.put(ctx.sender.clone(), Model { value: val }, cache);
                let n = String::from_utf8(ctx.sender.clone()).unwrap();
                ctx.dispatch_event("model", &[("created", &n)]);
                return Ok(());
            }
            Msgs::Inc => {
                let store = ModelStore::load(&self.name);
                match store.get(ctx.sender.clone(), cache) {
                    Some(mut m) => {
                        m.value += 1;
                        store.put(ctx.sender.clone(), m, cache);
                        let n = String::from_utf8(ctx.sender.clone()).unwrap();
                        ctx.dispatch_event("model", &[("inc", &n)]);
                        return Ok(());
                    }
                    _ => bail!("model for user not found"),
                }
            }
        }
    }

    fn handle_query(
        &self,
        path: &str,
        key: Vec<u8>,
        view: &StoreView,
    ) -> Result<Vec<u8>, anyhow::Error> {
        match path {
            "/" => {
                let store = ModelStore::load(&self.name);
                match store.query(key, view) {
                    Some(p) => Ok(p.try_to_vec().unwrap()),
                    None => bail!("Model not found for user"),
                }
            }
            _ => bail!("invalid query"),
        }
    }
}

// Test Authenticator that only recognizes 1 account. Set via AppBuilder
pub struct TestAuthenticator {
    pubkey: PublicKey,
}
impl TestAuthenticator {
    pub fn new(pubkey: PublicKey) -> Self {
        Self { pubkey }
    }
}
impl Authenticator for TestAuthenticator {
    fn validate(
        &self,
        tx: &SignedTransaction,
        _view: &StoreView,
    ) -> anyhow::Result<(), anyhow::Error> {
        // Check the signature
        ensure!(verify_tx_signature(tx, &self.pubkey), "bad signature");

        Ok(())
    }
}
