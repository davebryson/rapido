use anyhow::bail;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{Hash, PUBLIC_KEY_LENGTH};
use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    BinaryValue, ObjectHash,
};
use std::{borrow::Cow, convert::AsRef};

const ACCOUNT_STORE: &str = "rapido_account";

// Did format:
// base58(sha256(publickey))
// did:rapido:{did}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Default)]
pub(crate) struct DidAccount {
    pub did: Vec<u8>,
    pub nonce: u64,
    // Authentication Key
    pub pubkey: [u8; PUBLIC_KEY_LENGTH],
    pub revoked: bool,
}

impl DidAccount {
    pub fn increment_nonce(&self) -> Self {
        Self {
            did: self.did.clone(),
            nonce: self.nonce + 1,
            pubkey: self.pubkey,
            revoked: false,
        }
    }
}

impl BinaryValue for DidAccount {
    fn to_bytes(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, anyhow::Error> {
        DidAccount::try_from_slice(bytes.as_ref()).map_err(From::from)
    }
}

impl ObjectHash for DidAccount {
    fn object_hash(&self) -> Hash {
        exonum_crypto::hash(&BinaryValue::to_bytes(self))
    }
}

#[derive(Debug)]
pub(crate) struct AccountSchema<T: Access> {
    access: T,
}

impl<T: Access> AccountSchema<T> {
    pub fn new(access: T) -> Self {
        Self { access }
    }

    pub fn get_account(&self, did: Vec<u8>) -> Option<DidAccount> {
        self.access.get_proof_map(ACCOUNT_STORE).get(&did)
    }
}

impl<T: Access> AccountSchema<T>
where
    T::Base: RawAccessMut,
{
    pub fn increment_nonce(&mut self, did: Vec<u8>) -> Result<(), anyhow::Error> {
        match self.get_account(did) {
            Some(acct) => {
                self.update(acct.increment_nonce());
                Ok(())
            }
            None => bail!("no account"),
        }
    }

    pub fn update(&mut self, acct: DidAccount) {
        self.access
            .get_proof_map(ACCOUNT_STORE)
            .put(&acct.did.clone(), acct);
    }
}
