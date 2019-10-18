use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{
    impl_object_hash_for_binary_value, BinaryValue, Entry, ObjectAccess, ObjectHash, ProofMapIndex,
    RefMut,
};
use std::{borrow::Cow, convert::AsRef};

const APP_STATE_STORE: &str = "app_state_store";
const APP_ROOT_STORE: &str = "_app_root_table_";

// App state provide blockchain application information.  It's used to determine
// if a given node is in sync.  This information is not included in the overall app hash.
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

/// Make a composite key for the appstate store
pub fn app_root_key(r: &str, i: usize) -> Vec<u8> {
    let mut a: Vec<u8> = r.into();
    a.push(i as u8); // will fail if the index value is > 255
    a
}

pub struct AppRootStoreSchema<T: ObjectAccess>(T);
impl<T: ObjectAccess> AppRootStoreSchema<T> {
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    pub fn store(&self) -> RefMut<ProofMapIndex<T, Vec<u8>, Hash>> {
        self.0.get_object(APP_ROOT_STORE)
    }

    pub fn get_root_hash(&self) -> Hash {
        self.store().object_hash()
    }
}
