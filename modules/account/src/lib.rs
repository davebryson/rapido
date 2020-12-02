use anyhow::bail;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{hash, KeyPair, PublicKey, Seed};
use rapido_core::{AccountId, Store, StoreView};

//use crate::types::AccountId;

//pub mod dev;
//pub mod handler;

//pub use self::handler::authenticate;

#[macro_use]
extern crate rapido_core;

const PUBKEY_SIZE: usize = 32;

pub type PubKeyBytes = [u8; PUBKEY_SIZE];

pub fn generate_did(pk: PublicKey) -> String {
    let identifer =
        bs58::encode(exonum_crypto::hash(&pk.as_bytes()[..]).as_bytes().to_vec()).into_string();
    format!("did:rapido:{}", identifer)
}

fn generator(v: &str) -> Account {
    let pair = create_keypair(v);
    Account::create(pair.public_key())
}

pub fn create_keypair(v: &str) -> KeyPair {
    let seed = Seed::new(hash(v.as_bytes()).as_bytes());
    KeyPair::from_seed(&seed)
}

/// Return a list of Accounts to use for testing/development
pub fn generate_dev_accounts() -> Vec<Account> {
    vec![generator("/Dave"), generator("/Bob"), generator("/Alice")]
}

#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq, Clone)]
pub struct Account {
    id: AccountId, // base58(hash(pubkey)))
    nonce: u64,
    pubkey: PubKeyBytes,
}

impl Account {
    // Make an Id based on the base58 of hash of the pubkey
    pub fn create(pk: PublicKey) -> Self {
        Self {
            id: generate_did(pk),
            nonce: 0u64,
            pubkey: pk.as_bytes(),
        }
    }

    pub fn inc_nonce(&self) -> Self {
        Self {
            id: self.id.clone(),
            nonce: self.nonce + 1,
            pubkey: self.pubkey,
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    pub fn pubkey(&self) -> Option<PublicKey> {
        PublicKey::from_slice(&self.pubkey)
    }

    pub fn pubkey_as_hex(&self) -> String {
        format!("0x{:}", hex::encode(self.pubkey))
    }
}

impl_store_values!(Account);

pub(crate) struct AccountStore;
impl Store for AccountStore {
    type Key = String;
    type Value = Account;

    fn name(&self) -> String {
        "rapido.account.store".into()
    }
}

impl AccountStore {
    pub fn new() -> Self {
        Self {}
    }

    pub fn save(&self, account: Account, view: &mut StoreView) {
        self.put(account.id(), account, view)
    }

    pub fn get_account<I: Into<String>>(&self, id: I, view: &StoreView) -> Option<Account> {
        self.get(id.into(), view)
    }

    pub fn has_account<I: Into<String>>(&self, id: I, view: &StoreView) -> bool {
        self.get_account(id, view).is_some()
    }

    pub fn get_publickey<I: Into<String>>(&self, id: I, view: &StoreView) -> Option<String> {
        self.get_account(id, view)
            .and_then(|acct| Some(acct.pubkey_as_hex()))
    }

    pub fn get_nonce<I: Into<String>>(&self, id: I, view: &StoreView) -> Option<u64> {
        self.get_account(id, view)
            .and_then(|acct| Some(acct.nonce()))
    }
}

pub fn increment_nonce<I: Into<String>>(id: I, view: &mut StoreView) -> Result<(), anyhow::Error> {
    let store = AccountStore::new();
    match store.get_account(id, view) {
        Some(acct) => {
            store.save(acct.inc_nonce(), view);
            Ok(())
        }
        _ => bail!("Account not found"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
