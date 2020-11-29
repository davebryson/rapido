use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use exonum_merkledb::{
    access::{Access, AccessExt},
    BinaryValue, Database, ObjectHash, ProofMapIndex, TemporaryDB,
};
use std::{borrow::Cow, convert::AsRef};

#[macro_use]
extern crate rapido;

const RAPIDO_CORE_MAP: &'static str = "_rapido_core_map_";

type Cache = BTreeMap<Vec<u8>, Vec<u8>>;

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Default)]
pub struct StoreKey {
    prefix: String,
    key: Vec<u8>,
}

impl StoreKey {
    pub fn create(prefix: String, key: Vec<u8>) -> Self {
        Self { prefix, key }
    }
}

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Default)]
pub struct Person {
    name: String,
    age: u8,
}

impl_store_values!(StoreKey, Person);

// Cache should take snapshot
#[derive(Debug)]
pub struct CacheMap<T: Access> {
    cache: Cache,
    access: T,
}

impl<T: Access> CacheMap<T> {
    pub fn new(access: T) -> Self {
        Self {
            cache: Default::default(),
            access,
        }
    }

    fn get_store(&self) -> ProofMapIndex<T::Base, Vec<u8>, Vec<u8>> {
        self.access.get_proof_map(RAPIDO_CORE_MAP)
    }

    pub fn wrap(access: T, cache: Cache) -> Self {
        CacheMap { access, cache }
    }

    pub fn into_cache(self) -> Cache {
        self.cache
    }

    pub fn exists(&self, storename: String, key: &Vec<u8>) -> bool {
        let sk = StoreKey::create(storename, key.to_owned());
        self.cache.contains_key(&sk.to_bytes())
    }

    pub fn get(&self, key: &StoreKey) -> Option<&Vec<u8>> {
        self.cache.get(&key.to_bytes())
    }

    pub fn get_from_store(&self, key: &StoreKey) -> Option<Vec<u8>> {
        self.get_store().get(&key.to_bytes())
    }

    pub fn put(&mut self, key: StoreKey, value: impl BinaryValue) {
        self.cache.insert(key.to_bytes(), value.to_bytes());
    }

    // TODO:
    pub fn commit(&self) {}
}

// A store takes a cachmap as params to put,get, etc...
pub trait Store {
    type Value: BinaryValue;

    fn name(&self) -> String;

    fn put<T: Access>(&self, key: Vec<u8>, v: Self::Value, cache: &mut CacheMap<T>) {
        let sk = StoreKey::create(self.name(), key.to_owned());
        cache.put(sk, v)
    }

    fn get<T: Access>(&self, key: &Vec<u8>, cache: &mut CacheMap<T>) -> Option<Self::Value> {
        let sk = StoreKey::create(self.name(), key.to_owned());
        if let Some(v) = cache.get(&sk) {
            return match Self::Value::from_bytes(Cow::Owned(v.clone())) {
                Ok(r) => Some(r),
                _ => None,
            };
        }

        if let Some(v) = cache.get_from_store(&sk) {
            return match Self::Value::from_bytes(Cow::Owned(v.clone())) {
                Ok(r) => Some(r),
                _ => None,
            };
        }

        None
    }
}

// Add BinaryKey as a type also?
pub struct MyStore;
impl Store for MyStore {
    type Value = Person;

    fn name(&self) -> String {
        "mystore".into()
    }
}

#[test]
fn test_store_trait() {
    let db: Box<dyn Database> = Box::new(TemporaryDB::new());
    let sn = db.snapshot();
    let mut c1 = CacheMap::new(&sn);

    let store = MyStore {};
    store.put(
        vec![1],
        Person {
            name: "bob".into(),
            age: 1u8,
        },
        &mut c1,
    );

    store.put(
        vec![2],
        Person {
            name: "carl".into(),
            age: 2u8,
        },
        &mut c1,
    );

    // This passes because we haven't committed
    assert!(store.get(&vec![1], &mut c1).is_some());
    assert!(store.get(&vec![11], &mut c1).is_none());

    let t = c1.into_cache();
    println!("{:?}", t);
}
