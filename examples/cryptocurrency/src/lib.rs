use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{
    impl_object_hash_for_binary_value, BinaryValue, Fork, ObjectAccess, ObjectHash, ProofMapIndex,
    RefMut, Snapshot,
};
use rapido::{AccountAddress, QueryResult, Service, Transaction, TxResult};
use std::{
    borrow::Cow,
    convert::AsRef,
    convert::TryFrom,
    io::{Error, ErrorKind},
};

pub const CRYPTO_SERVICE_ROUTE_NAME: &str = "cryptoapp";

/** Transactions  */
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
pub struct CreateAcctTx; // txid=0
impl CreateAcctTx {
    pub fn into_boxed_tx(payload: &[u8]) -> Result<Box<dyn Transaction>, Error> {
        let msg = Self::try_from_slice(payload)?;
        Ok(Box::new(msg))
    }
}

impl Transaction for CreateAcctTx {
    // Free accounts! it's an example...
    fn execute(&self, sender: AccountAddress, fork: &Fork) -> TxResult {
        let mut store = SchemaStore::new(fork).state();
        if store.contains(&sender) {
            return TxResult::error(12, "account already exists");
        }

        // Create the account with a balance
        store.put(
            &sender,
            Account {
                account: sender.clone(),
                balance: 10u64,
            },
        );

        TxResult::ok()
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
pub struct DepositTx(pub u64); // txid=1
impl DepositTx {
    pub fn into_boxed_tx(payload: &[u8]) -> Result<Box<dyn Transaction>, Error> {
        let msg = Self::try_from_slice(payload)?;
        Ok(Box::new(msg))
    }
}

impl Transaction for DepositTx {
    // Only you can deposit into your account
    fn execute(&self, sender: AccountAddress, fork: &Fork) -> TxResult {
        let deposit = self.0;

        let mut store = SchemaStore::new(fork).state();
        if !store.contains(&sender) {
            return TxResult::error(20, "account doesn't exist");
        }

        let mut account = store.get(&sender).unwrap();
        account.balance += deposit;
        store.put(&sender, account);

        TxResult::ok()
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
pub struct TransferTx(pub AccountAddress, pub u64); // txid=2
impl TransferTx {
    pub fn into_boxed_tx(payload: &[u8]) -> Result<Box<dyn Transaction>, Error> {
        let msg = Self::try_from_slice(payload)?;
        Ok(Box::new(msg))
    }
}

impl Transaction for TransferTx {
    fn execute(&self, sender: AccountAddress, fork: &Fork) -> TxResult {
        let recipient = self.0;
        let transfer_amount = self.1;
        let mut store = SchemaStore::new(fork).state();

        let mut sender_account = store.get(&sender).unwrap();
        if sender_account.balance < transfer_amount {
            return TxResult::error(25, "Insufficient funds!");
        }

        let mut recip_account = store.get(&recipient).unwrap();

        sender_account.balance -= transfer_amount;
        recip_account.balance += transfer_amount;

        store.put(&sender, sender_account);
        store.put(&recipient, recip_account);
        TxResult::ok()
    }
}

// Storage Model
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
pub struct Account {
    pub account: AccountAddress,
    pub balance: u64,
}

impl BinaryValue for Account {
    fn to_bytes(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        Account::try_from_slice(bytes.as_ref()).map_err(From::from)
    }
}

// Exonum db macro to add hash()
impl_object_hash_for_binary_value! { Account }

// State store
pub struct SchemaStore<T: ObjectAccess>(T);
impl<T: ObjectAccess> SchemaStore<T> {
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    pub fn state(&self) -> RefMut<ProofMapIndex<T, AccountAddress, Account>> {
        self.0.get_object(CRYPTO_SERVICE_ROUTE_NAME)
    }
}

// Service
pub struct CryptocurrencyService;
impl Service for CryptocurrencyService {
    fn route(&self) -> &'static str {
        CRYPTO_SERVICE_ROUTE_NAME
    }

    fn decode_tx(&self, txid: u8, payload: Vec<u8>) -> Result<Box<dyn Transaction>, Error> {
        let bits = &payload[..];
        match txid {
            0 => CreateAcctTx::into_boxed_tx(bits),
            1 => DepositTx::into_boxed_tx(bits),
            2 => TransferTx::into_boxed_tx(bits),
            _ => Err(Error::new(ErrorKind::Other, "msg not found")),
        }
    }

    fn query(&self, _path: &str, key: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> QueryResult {
        let acct = AccountAddress::try_from(key);
        if acct.is_err() {
            return QueryResult::error(22);
        }

        let schema = SchemaStore::new(snapshot);
        if let Some(account) = schema.state().get(&acct.unwrap()) {
            let bits = account.into_bytes();
            return QueryResult::ok(bits);
        }
        QueryResult::error(1)
    }

    fn root_hash(&self, fork: &Fork) -> Hash {
        let schema = SchemaStore::new(fork);
        schema.state().object_hash()
    }
}
