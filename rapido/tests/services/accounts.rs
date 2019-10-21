//! Basic Account Service
//! Provides the ability to:
//! - Load initial accounts from a serialized genesis file
//! - Query an Account by ID
//! - Tranfer funds via the store
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{Hash, PUBLIC_KEY_LENGTH};
use exonum_merkledb::{
    impl_object_hash_for_binary_value, BinaryValue, Fork, ObjectAccess, ObjectHash, ProofMapIndex,
    RefMut, Snapshot,
};
use failure::ensure;
use rapido::{verify_tx_signature, QueryResult, Service, SignedTransaction, Transaction, TxResult};
use std::{
    borrow::Cow,
    convert::AsRef,
    io::{Error, ErrorKind},
};

pub const ACCOUNT_SERVICE_ROUTE: &str = "accounts_service";

/// Container for Genesis Account Data
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct GenesisAccounts {
    pub accounts: Vec<(Vec<u8>, u8, [u8; PUBLIC_KEY_LENGTH])>,
}

impl GenesisAccounts {
    pub fn new() -> Self {
        Self {
            accounts: Vec::new(),
        }
    }

    pub fn add(&mut self, id: Vec<u8>, balance: u8, pk: [u8; PUBLIC_KEY_LENGTH]) {
        self.accounts.push((id, balance, pk));
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }

    pub fn decode(data: &Vec<u8>) -> Result<Self, Error> {
        Self::try_from_slice(&data[..])
    }
}

/// Authentication handler used for check_tx
pub fn authenticate_sender(tx: &SignedTransaction, snapshot: &Box<dyn Snapshot>) -> TxResult {
    let store = AccountStore::new(snapshot);
    match store
        .fetch(&tx.sender.clone())
        .filter(|acct| verify_tx_signature(tx, &acct.get_public_key()))
    {
        Some(_) => TxResult::ok(),
        None => TxResult::error(10, "Account not found or bad signature"),
    }
}

/// Account Model
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct Account {
    pub id: Vec<u8>,
    pub balance: u8, // intentional for testing
    public_key_bits: [u8; PUBLIC_KEY_LENGTH],
}

impl Account {
    pub fn new(id: Vec<u8>, balance: u8, public_key_bits: [u8; PUBLIC_KEY_LENGTH]) -> Self {
        Self {
            id,
            balance,
            public_key_bits,
        }
    }

    pub fn get_public_key(&self) -> exonum_crypto::PublicKey {
        exonum_crypto::PublicKey::new(self.public_key_bits)
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }

    pub fn decode(data: Vec<u8>) -> Self {
        Self::try_from_slice(&data[..]).unwrap()
    }

    pub fn debit(&mut self, amount: u8) -> Result<(), failure::Error> {
        if self.balance < amount {
            return Err(failure::err_msg("Insufficient funds"));
        }
        self.balance -= amount;
        Ok(())
    }

    pub fn credit(&mut self, amount: u8) {
        self.balance += amount;
    }
}

// Exonum db requirement for storage
impl BinaryValue for Account {
    fn to_bytes(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        Account::try_from_slice(bytes.as_ref()).map_err(From::from)
    }
}
impl_object_hash_for_binary_value! { Account }

// Storage
pub struct AccountStore<T: ObjectAccess>(T);
impl<T: ObjectAccess> AccountStore<T> {
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    fn accounts(&self) -> RefMut<ProofMapIndex<T, Vec<u8>, Account>> {
        self.0.get_object("_accounts_store_")
    }

    fn create(&self, id: Vec<u8>, balance: u8, pubkey: [u8; PUBLIC_KEY_LENGTH]) {
        if let None = self.fetch(&id) {
            self.accounts()
                .put(&id, Account::new(id.clone(), balance, pubkey));
        }
    }

    pub fn fetch(&self, id: &Vec<u8>) -> Option<Account> {
        self.accounts().get(id)
    }

    pub fn transfer(
        &self,
        sender: Vec<u8>,
        recip: Vec<u8>,
        amount: u8,
    ) -> Result<(), failure::Error> {
        let from_account = self.fetch(&sender);
        let to_account = self.fetch(&recip);

        ensure!(from_account.is_some(), "sender account not found");
        ensure!(to_account.is_some(), "recipient account not found");
        let mut fa = from_account.unwrap();
        let mut ta = to_account.unwrap();

        fa.debit(amount)?;
        ta.credit(amount);

        self.accounts().put(&sender, fa);
        self.accounts().put(&recip, ta);

        Ok(())
    }
}

pub struct AccountService;
impl Service for AccountService {
    fn route(&self) -> &'static str {
        ACCOUNT_SERVICE_ROUTE
    }

    fn genesis(&self, fork: &Fork, data: Option<&Vec<u8>>) -> TxResult {
        match data.and_then(|raw| GenesisAccounts::decode(raw).ok()) {
            Some(ga) => {
                let store = AccountStore::new(fork);
                for (id, bal, pk) in &ga.accounts {
                    store.create(id.clone(), *bal, *pk);
                }
            }
            None => return TxResult::error(1, "problem extracting genesis accounts"),
        }
        TxResult::ok()
    }

    fn decode_tx(
        &self,
        _txid: u8,
        _payload: Vec<u8>,
    ) -> Result<Box<dyn Transaction>, std::io::Error> {
        // We don't process any message right now
        Err(Error::new(ErrorKind::Other, "not implemented!"))
    }

    fn query(&self, path: &str, key: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> QueryResult {
        if path == "/account" {
            let store = AccountStore::new(snapshot);
            return match store.fetch(&key) {
                Some(acct) => QueryResult::ok(acct.encode()),
                None => QueryResult::error(2),
            };
        }

        QueryResult::error(2)
    }

    fn store_hashes(&self, fork: &Fork) -> Vec<Hash> {
        let store = AccountStore::new(fork);
        vec![store.accounts().object_hash()]
    }
}
