use std::collections::HashMap;

use borsh::{BorshDeserialize, BorshSerialize};
use exonum_merkledb::{
    access::{Access, AccessExt},
    BinaryValue, Fork, ProofMapIndex, Snapshot,
};

use exonum_crypto::Hash;

use std::borrow::Cow;

const RAPIDO_CORE_MAP: &'static str = "_rapido_core_map_";

//pub type Cache = BTreeMap<Vec<u8>, Vec<u8>>;
pub type Cache = HashMap<Hash, Vec<u8>>;

// Could use hash for this and Hash as key in main table
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Default)]
pub struct StoreKey<K> {
    prefix: String,
    key: K,
}

impl<K> StoreKey<K>
where
    K: BorshSerialize + BorshDeserialize,
{
    pub fn create<P: Into<String>>(prefix: P, key: K) -> Self {
        Self {
            prefix: prefix.into(),
            key,
        }
    }

    pub fn hash(&self) -> Hash {
        exonum_crypto::hash(&self.try_to_vec().unwrap())
    }
}

// MOVE TO SCHEMA
pub(crate) fn get_store<T: Access>(access: T) -> ProofMapIndex<T::Base, Hash, Vec<u8>> {
    access.get_proof_map(RAPIDO_CORE_MAP)
}

#[derive(Debug)]
pub struct CacheMap<'a> {
    cache: Cache,
    access: &'a Box<dyn Snapshot>,
}

impl<'a> CacheMap<'a> {
    pub fn wrap(db: &'a Box<dyn Snapshot>, cache: Cache) -> Self {
        CacheMap {
            access: db,
            cache: cache,
        }
    }

    pub fn into_cache(self) -> Cache {
        self.cache
    }

    pub fn exists(&self, key: &Hash) -> bool {
        self.cache.contains_key(&key)
    }

    pub fn get(&self, key: &Hash) -> Option<&Vec<u8>> {
        self.cache.get(&key)
    }

    pub fn get_from_store(&self, key: &Hash) -> Option<Vec<u8>> {
        get_store(self.access).get(&key)
    }

    pub fn put(&mut self, key: Hash, value: impl BinaryValue) {
        self.cache.insert(key, value.to_bytes());
    }

    pub fn commit(&self, fork: &Fork) {
        let mut store = get_store(fork);
        for (k, v) in &self.cache {
            store.put(k, v.to_owned());
        }
    }
}

// A store takes a cachmap as params to put,get, etc...
// TODO: Add: remove, get_proof, contains
pub trait Store {
    type Key: BorshSerialize + BorshDeserialize;
    type Value: BinaryValue;

    fn name(&self) -> String;

    fn put(&self, key: Self::Key, v: Self::Value, cache: &mut CacheMap) {
        let hash = StoreKey::create(self.name(), key).hash();
        cache.put(hash, v)
    }

    fn get(&self, key: Self::Key, cache: &mut CacheMap) -> Option<Self::Value> {
        let hash = StoreKey::create(self.name(), key).hash();
        if let Some(v) = cache.get(&hash) {
            return match Self::Value::from_bytes(Cow::Owned(v.clone())) {
                Ok(r) => Some(r),
                _ => None,
            };
        }

        if let Some(v) = cache.get_from_store(&hash) {
            return match Self::Value::from_bytes(Cow::Owned(v.clone())) {
                Ok(r) => Some(r),
                _ => None,
            };
        }

        None
    }

    fn query(&self, key: Self::Key, snapshot: &Box<dyn Snapshot>) -> Option<Self::Value> {
        let hash = StoreKey::create(self.name(), key).hash();
        let store = get_store(snapshot);
        if let Some(v) = store.get(&hash) {
            return match Self::Value::from_bytes(Cow::Owned(v.clone())) {
                Ok(r) => Some(r),
                _ => None,
            };
        }
        None
    }

    //fn remove(&self, key: &Vec<u8>, cache: &mut CacheMap) {}

    //fn get_proof(&self, key: &Vec<u8>);

    //fn exists(&self, key: &Vec<u8>);
}
