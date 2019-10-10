use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{
    impl_object_hash_for_binary_value, BinaryValue, Fork, ObjectAccess, ObjectHash, ProofMapIndex,
    RefMut, Snapshot, TemporaryDB,
};
use std::sync::Arc;
use std::{borrow::Cow, convert::AsRef};

use rapido::{AccountId, AppBuilder, QueryResult, Service, Transaction, TxResult};

pub const CRYPTO_SERVICE_ROUTE_NAME: &str = "cryptoapp";

/** Messages  */
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
pub struct CreateAcctTx {
    pub account: AccountId,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
pub struct DepositTx {
    account: AccountId,
    amount: u64,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
pub struct TransferTx {
    recipient: AccountId,
    amount: u64,
}

// Storage
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
pub struct Account {
    pub name: AccountId,
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

impl_object_hash_for_binary_value! { Account }

pub struct SchemaStore<T: ObjectAccess>(T);
impl<T: ObjectAccess> SchemaStore<T> {
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    pub fn state(&self) -> RefMut<ProofMapIndex<T, AccountId, Account>> {
        self.0.get_object(CRYPTO_SERVICE_ROUTE_NAME)
    }
}

pub struct CryptocurrencyService;
impl Service for CryptocurrencyService {
    fn route(&self) -> String {
        CRYPTO_SERVICE_ROUTE_NAME.into()
    }

    fn execute(&self, tx: &Transaction, fork: &Fork) -> TxResult {
        match tx.msgtype {
            0 => on_account_create(&tx.msg, fork),
            1 => on_account_deposit(&tx.msg, fork),
            2 => on_account_transfer(tx.signer.clone(), &tx.msg, fork),
            _ => TxResult::error(10, "unknown message"),
        }
    }

    fn query(&self, _path: String, key: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> QueryResult {
        on_account_query(key, snapshot)
    }

    fn root_hash(&self, fork: &Fork) -> Hash {
        let schema = SchemaStore::new(fork);
        schema.state().object_hash()
    }
}

// ** Handlers **

fn on_account_query(key: Vec<u8>, snapshot: &Box<dyn Snapshot>) -> QueryResult {
    let schema = SchemaStore::new(snapshot);
    if let Some(account) = schema.state().get(&key) {
        let bits = account.into_bytes();
        return QueryResult::ok(bits);
    }
    QueryResult::error(1)
}

fn on_account_create(raw_msg: &Vec<u8>, fork: &Fork) -> TxResult {
    let m = CreateAcctTx::try_from_slice(&raw_msg[..]);
    if m.is_err() {
        return TxResult::error(13, "Error parsing message");
    }
    let account_name = m.unwrap().account;

    let mut store = SchemaStore::new(fork).state();
    if store.contains(&account_name) {
        return TxResult::error(12, "account already exists");
    }

    store.put(
        &account_name,
        Account {
            name: account_name.clone(),
            balance: 10u64,
        },
    );

    TxResult::ok()
}

// rules:  anyone can deposit to an existing account
fn on_account_deposit(raw_msg: &Vec<u8>, fork: &Fork) -> TxResult {
    let msg = DepositTx::try_from_slice(&raw_msg[..]);
    if msg.is_err() {
        return TxResult::error(14, "Error parsing message");
    }
    let deposit = msg.unwrap();

    let mut store = SchemaStore::new(fork).state();
    if store.contains(&deposit.account) {
        return TxResult::error(20, "account doesn't exist");
    }

    let mut account = store.get(&deposit.account).unwrap();
    account.balance += deposit.amount;

    store.put(&deposit.account, account);

    TxResult::ok()
}

fn on_account_transfer(sender: Vec<u8>, raw_msg: &Vec<u8>, fork: &Fork) -> TxResult {
    let m = TransferTx::try_from_slice(&raw_msg[..]);
    if m.is_err() {
        return TxResult::error(15, "Error parsing message");
    }

    let transfer = m.unwrap();
    let mut store = SchemaStore::new(fork).state();

    let mut sender_account = store.get(&sender).unwrap();
    if sender_account.balance < transfer.amount {
        return TxResult::error(25, "Insufficient funds!");
    }

    let mut recip_account = store.get(&transfer.recipient).unwrap();

    sender_account.balance -= transfer.amount;
    recip_account.balance += transfer.amount;

    store.put(&sender, sender_account);
    store.put(&transfer.recipient, recip_account);

    TxResult::ok()
}


// Main Application!
fn main() {
    let db = Arc::new(TemporaryDB::new());
    let node = AppBuilder::new(db)
        .add_service(Box::new(CryptocurrencyService {}))
        .finish();
    abci::run_local(node);
}