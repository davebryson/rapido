//! Proof of DNA Service.  Register your DNA to prove it's you!
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{Fork, ObjectAccess, ObjectHash, ProofMapIndex, RefMut, Snapshot};
use rapido::{RapidoError, Service, Transaction};

use super::accounts::AccountStore;

pub const PODNA_SERVICE_ROUTE: &str = "proof_of_dna_service";
const REGISTRATION_COST: u8 = 5; // The cost to register

// Storage
pub struct DNAStore<T: ObjectAccess>(T);
impl<T: ObjectAccess> DNAStore<T> {
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    fn acct_to_dna(&self) -> RefMut<ProofMapIndex<T, Vec<u8>, Hash>> {
        self.0.get_object("_podna_a_to_d_store_")
    }

    fn dna_to_acct(&self) -> RefMut<ProofMapIndex<T, Hash, Vec<u8>>> {
        self.0.get_object("_podna_d_to_a_store_")
    }

    pub fn register_dna(&self, acct: Vec<u8>, dna: Hash) -> Result<(), RapidoError> {
        if let Some(_) = self.dna_to_acct().get(&dna) {
            return Err(RapidoError::from("DNA already registered!"));
        }
        self.acct_to_dna().put(&acct, dna);
        self.dna_to_acct().put(&dna, acct);
        Ok(())
    }
}

pub struct DNAService;
impl Service for DNAService {
    fn route(&self) -> &'static str {
        PODNA_SERVICE_ROUTE
    }

    fn decode_tx(&self, _txid: u8, payload: Vec<u8>) -> Result<Box<dyn Transaction>, RapidoError> {
        let msg = RegisterTx::try_from_slice(&payload[..])?;
        Ok(Box::new(msg))
    }

    fn query(
        &self,
        path: &str,
        key: Vec<u8>,
        snapshot: &Box<dyn Snapshot>,
    ) -> Result<Vec<u8>, RapidoError> {
        if path != "/dna" {
            return Err(RapidoError::from("bad path for query"));
        }
        let store = DNAStore::new(snapshot);
        match store.acct_to_dna().get(&key) {
            Some(value) => Ok(value[..].to_vec()),
            None => Err(RapidoError::from("Account not found")),
        }
    }

    fn store_hashes(&self, fork: &Fork) -> Vec<Hash> {
        let store = DNAStore::new(fork);
        vec![
            store.acct_to_dna().object_hash(),
            store.dna_to_acct().object_hash(),
        ]
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct RegisterTx(pub Vec<u8>, [u8; 32]);
impl RegisterTx {
    pub fn new(to_pay: Vec<u8>, dna: [u8; 32]) -> Self {
        Self(to_pay, dna)
    }
}

impl Transaction for RegisterTx {
    fn execute(&self, sender: Vec<u8>, fork: &Fork) -> Result<(), RapidoError> {
        let store = DNAStore::new(fork);
        let accounts = AccountStore::new(fork);

        if let None = accounts.fetch(&sender) {
            return Err(RapidoError::from("No account found"));
        }
        let acct = accounts.fetch(&sender).unwrap();
        store
            .register_dna(acct.id, Hash::new(self.1))
            .and_then(|_| accounts.transfer(sender, self.0.clone(), REGISTRATION_COST))
    }
}
