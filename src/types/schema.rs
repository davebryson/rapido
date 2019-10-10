use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{
    impl_object_hash_for_binary_value, BinaryValue, Entry, ObjectAccess, ObjectHash, RefMut,
};
use std::{borrow::Cow, convert::AsRef};

const APP_STATE_STORE: &str = "app_state_store";

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
pub struct AppState {
    pub version: i64,
    pub hash: Vec<u8>,
}

impl BinaryValue for AppState {
    fn to_bytes(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        AppState::try_from_slice(bytes.as_ref()).map_err(From::from)
    }
}

impl_object_hash_for_binary_value! { AppState }

pub struct AppStateSchema<T: ObjectAccess>(T);

impl<T: ObjectAccess> AppStateSchema<T> {
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    pub fn app_state(&self) -> RefMut<Entry<T, AppState>> {
        self.0.get_object(APP_STATE_STORE)
    }
}
