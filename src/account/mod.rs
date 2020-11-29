use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{PublicKey, PUBLIC_KEY_LENGTH};
use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    BinaryValue, ObjectHash, ProofMapIndex, Snapshot,
};
use std::{borrow::Cow, convert::AsRef};

use super::{verify_tx_signature, SignedTransaction};

const ACCOUNT_STORE: &str = "rapido_account";

// Did format:
// base58(sha256(publickey))
// did:rapido:{did}

// Mut Actions:
// create_account (create)
// change_master (change_master)
// revoke (revoke)
// increment_nonce (inc_nonce)

// Read:
// get_account
// exists(did)

// What should be callable from other AppModules?

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Default)]
pub struct DidAccount {
    pub did: Vec<u8>,
    pub nonce: u64,
    // Authentication Key
    pub pubkey: [u8; PUBLIC_KEY_LENGTH],
    pub revoked: bool,
}

// Make it a stored value
impl_store_values!(DidAccount);

#[derive(Debug)]
pub(crate) struct AccountSchema<T: Access> {
    access: T,
}

// methods:
// contains_key -> bool
// get(key) -> T
impl<T: Access> AccountSchema<T> {
    pub fn new(access: T) -> Self {
        Self { access }
    }

    pub fn account(&self) -> ProofMapIndex<T::Base, Vec<u8>, DidAccount> {
        self.access.get_proof_map(ACCOUNT_STORE)
    }

    pub fn get(&self, did: Vec<u8>) -> Option<DidAccount> {
        self.account().get(&did)
    }
}

impl<T: Access> AccountSchema<T>
where
    T::Base: RawAccessMut,
{
    pub fn insert(&mut self, k: Vec<u8>, v: DidAccount) {
        self.account().put(&k, v);
    }

    pub fn remove(&mut self, k: Vec<u8>) {
        self.account().remove(&k);
    }
}

pub struct AccountManager;
impl AccountManager {
    pub fn get_account(k: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> Option<DidAccount> {
        let store = AccountSchema::new(snapshot);
        store.get(k)
    }

    pub fn nonce(k: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> Option<u64> {
        let store = AccountSchema::new(snapshot);
        match store.get(k) {
            Some(acct) => Some(acct.nonce),
            _ => None,
        }
    }
}

pub fn account_authentication(
    tx: &SignedTransaction,
    snapshot: &Box<dyn Snapshot>,
) -> Result<(), anyhow::Error> {
    let acct = AccountManager::get_account(tx.sender.clone(), snapshot).unwrap();
    let pkbytes = PublicKey::from_slice(&acct.pubkey).unwrap();

    // Check signature
    if !verify_tx_signature(tx, &pkbytes) {
        anyhow::bail!("bad signature")
    }

    // TODO: Nonce check is tricky!  If the person submits several transactions to
    // the pool at once, where/when do you inc the nonce?
    // check nonce
    if tx.nonce != acct.nonce {
        anyhow::bail!("bad nonce")
    }

    Ok(())
}
