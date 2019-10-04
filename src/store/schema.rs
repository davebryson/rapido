use exonum_crypto::Hash;
use exonum_merkledb::{
    impl_object_hash_for_binary_value, BinaryValue, Entry, ObjectAccess, ObjectHash, ProofMapIndex,
    RefMut,
};
use serde_derive::{Deserialize, Serialize};
use std::{borrow::Cow, convert::AsRef};

/// Experiments with schema
///
const RAPIDO_STORE: &str = "rapido_store";
const APP_STATE_STORE: &str = "app_state_store";

pub type StoreKey = Vec<u8>;
pub type StoreValue = Vec<u8>;

/// Used to store application state in the db.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct AppState {
    pub version: i64,
    pub hash: Vec<u8>,
}

impl BinaryValue for AppState {
    fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        bincode::deserialize(bytes.as_ref()).map_err(From::from)
    }
}

impl_object_hash_for_binary_value! { AppState }

pub struct Schema<T: ObjectAccess>(T);

impl<T: ObjectAccess> Schema<T> {
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    pub fn state(&self) -> RefMut<ProofMapIndex<T, StoreKey, StoreValue>> {
        self.0.get_object(RAPIDO_STORE)
    }

    pub fn app_state(&self) -> RefMut<Entry<T, AppState>> {
        self.0.get_object(APP_STATE_STORE)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use exonum_merkledb::{Database, TemporaryDB};

    #[test]
    fn test_nschema() {
        let db = TemporaryDB::new();

        {
            let snap = db.snapshot();
            let schema = Schema::new(&snap);
            let noacct = schema.state().get(&vec![1]);
            assert!(noacct.is_none());
        }

        {
            let fork = db.fork();
            let schema = Schema::new(&fork);
            schema.state().put(&vec![1], vec![5, 5, 5]);
            schema.state().put(&vec![2], vec![5, 5, 5]);

            let h = schema.state().object_hash();
            println!("{:}", h);

            let r = db.merge(fork.into_patch());
            assert!(r.is_ok());
        }

        {
            let snap = db.snapshot();
            let schema = Schema::new(&snap);
            let a1 = schema.state().get(&vec![1]);
            let a2 = schema.state().get(&vec![1]);
            assert_eq!(vec![5, 5, 5], a1.unwrap());
            assert_eq!(vec![5, 5, 5], a2.unwrap());
        }
    }
}
