pub use self::cryptocurrency::{CreateAcctTx, DepositTx, TransferTx};

use exonum_crypto::Hash;
use exonum_merkledb::{
    impl_object_hash_for_binary_value, BinaryValue, Fork, ObjectAccess, ObjectHash, ProofMapIndex,
    RefMut, Snapshot,
};
use protobuf::error::{ProtobufError, ProtobufResult};
use protobuf::Message;
use serde_derive::{Deserialize, Serialize};
use std::{borrow::Cow, convert::AsRef};

use rapido::{FromProtoBytes, IntoProtoBytes, QueryResult, Service, Tx, TxResult};

mod cryptocurrency;

pub const CRYPTO_SERVICE_ROUTE_NAME: &str = "cryptoapp";

impl IntoProtoBytes<CreateAcctTx> for CreateAcctTx {
    fn into_proto_bytes(self) -> ProtobufResult<Vec<u8>> {
        self.write_to_bytes()
    }
}
impl FromProtoBytes<CreateAcctTx> for CreateAcctTx {
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ProtobufError> {
        protobuf::parse_from_bytes::<Self>(bytes)
    }
}

impl IntoProtoBytes<DepositTx> for DepositTx {
    fn into_proto_bytes(self) -> ProtobufResult<Vec<u8>> {
        self.write_to_bytes()
    }
}
impl FromProtoBytes<DepositTx> for DepositTx {
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ProtobufError> {
        protobuf::parse_from_bytes::<Self>(bytes)
    }
}

impl IntoProtoBytes<TransferTx> for TransferTx {
    fn into_proto_bytes(self) -> ProtobufResult<Vec<u8>> {
        self.write_to_bytes()
    }
}
impl FromProtoBytes<TransferTx> for TransferTx {
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ProtobufError> {
        protobuf::parse_from_bytes::<Self>(bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Account {
    pub name: String,
    pub balance: u64,
}

impl BinaryValue for Account {
    fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        bincode::deserialize(bytes.as_ref()).map_err(From::from)
    }
}

impl_object_hash_for_binary_value! { Account }

pub struct SchemaStore<T: ObjectAccess>(T);
impl<T: ObjectAccess> SchemaStore<T> {
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    pub fn state(&self) -> RefMut<ProofMapIndex<T, String, Account>> {
        self.0.get_object(CRYPTO_SERVICE_ROUTE_NAME)
    }
}

pub struct CryptocurrencyService;
impl Service for CryptocurrencyService {
    fn route(&self) -> String {
        CRYPTO_SERVICE_ROUTE_NAME.into()
    }

    fn execute(&self, tx: &Tx, fork: &Fork) -> TxResult {
        match tx.msgtype {
            0 => on_account_create(&tx.msg, fork),
            1 => on_account_deposit(&tx.msg, fork),
            2 => on_account_transfer(tx.sender.clone(), &tx.msg, fork),
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
    let account_name = String::from_utf8(key).unwrap();
    let schema = SchemaStore::new(snapshot);
    if let Some(account) = schema.state().get(&account_name) {
        // NOTE: This uses bincode
        let bits = account.into_bytes();
        return QueryResult::ok(bits);
    }
    QueryResult::error(1)
}

fn on_account_create(raw_msg: &Vec<u8>, fork: &Fork) -> TxResult {
    let m = CreateAcctTx::from_proto_bytes(&raw_msg[..]);
    if m.is_err() {
        return TxResult::error(13, "Error parsing message");
    }
    let account_name = m.unwrap().name;

    let mut store = SchemaStore::new(fork).state();
    if store.contains(&account_name) {
        return TxResult::error(12, "account already exists");
    }

    store.put(
        &account_name,
        Account {
            name: account_name.clone(),
            balance: 0u64,
        },
    );

    TxResult::ok()
}

// rules:  anyone can deposit to an existing account
fn on_account_deposit(raw_msg: &Vec<u8>, fork: &Fork) -> TxResult {
    let msg = DepositTx::from_proto_bytes(&raw_msg[..]);
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
    let m = TransferTx::from_proto_bytes(&raw_msg[..]);
    if m.is_err() {
        return TxResult::error(15, "Error parsing message");
    }

    let s = String::from_utf8(sender);
    if s.is_err() {
        return TxResult::error(21, "Error parsing sender");
    }
    let sender_name = s.unwrap();

    let transfer = m.unwrap();
    let mut store = SchemaStore::new(fork).state();

    let mut sender_account = store.get(&sender_name).unwrap();
    if sender_account.balance < transfer.amount {
        return TxResult::error(25, "Insufficient funds!");
    }

    let mut recip_account = store.get(&transfer.recipient).unwrap();

    sender_account.balance -= transfer.amount;
    recip_account.balance += transfer.amount;

    store.put(&sender_name, sender_account);
    store.put(&transfer.recipient, recip_account);

    TxResult::ok()
}
