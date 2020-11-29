// Implement a cache for store access
// Flow...
// Check:
// Copy last check_cache into new cache
// pass snapshot
// set global check cache to new one...
// Cache should only ever use a snapshot

use std::collections::BTreeMap;
use std::marker::PhantomData;

use borsh::{BorshDeserialize, BorshSerialize};
use exonum_merkledb::{
    access::{Access, AccessExt},
    BinaryValue, Database, ObjectHash, ProofMapIndex, TemporaryDB,
};
use std::{borrow::Cow, convert::AsRef};

#[macro_use]
extern crate rapido;

type Cache = BTreeMap<Vec<u8>, Vec<u8>>;

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Default)]
pub struct StoreKey {
    prefix: String,
    key: Vec<u8>,
}

#[derive(Debug)]
pub struct CacheMap {
    cache: Cache,
}

impl CacheMap {
    pub fn new() -> Self {
        Self {
            cache: Default::default(),
        }
    }

    pub fn wrap(cache: Cache) -> Self {
        CacheMap { cache }
    }

    pub fn into_cache(self) -> Cache {
        self.cache
    }

    pub fn exists(&self, key: &Vec<u8>) -> bool {
        self.cache.contains_key(key)
    }

    // And here's the problem: If it's not in the cache,
    // *which* schema do you query???
    pub fn get(&self, key: &Vec<u8>) -> Option<&Vec<u8>> {
        self.cache.get(key)
    }

    pub fn put(&mut self, key: Vec<u8>, value: impl BinaryValue) {
        self.cache.insert(key, value.to_bytes());
    }

    pub fn put_with_prefix(&mut self, prefix: &str, key: Vec<u8>, value: impl BinaryValue) {
        let store_key = StoreKey {
            prefix: prefix.into(),
            key,
        };

        self.cache.insert(store_key.to_bytes(), value.to_bytes());
    }
}

trait Storage {
    type Val: BinaryValue;

    fn name() -> String;

    fn get(&self, key: Vec<u8>) -> Self::Val;
}

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Default)]
pub struct Person {
    name: String,
    age: u8,
}

impl_store_values!(Person, StoreKey);

pub struct StoreWrapper<'a, T: Access, V: BinaryValue> {
    appname: String,
    cache: &'a mut CacheMap,
    access: T,
    phantom: PhantomData<V>,
}

impl<'a, T: Access, V: BinaryValue> StoreWrapper<'a, T, V> {
    pub fn new(access: T, appname: String, cache: &'a mut CacheMap) -> Self {
        Self {
            access,
            appname,
            cache,
            phantom: PhantomData,
        }
    }

    fn get_store(&self) -> ProofMapIndex<T::Base, Vec<u8>, V> {
        self.access.get_proof_map(self.appname.clone())
    }

    pub fn put(&mut self, k: Vec<u8>, v: V) {
        // Need a prefixed key!
        self.cache.put(k, v)
    }

    pub fn get(&self, key: &Vec<u8>) -> Option<V> {
        // check cache
        if self.cache.exists(key) {
            let r = self.cache.get(&key).unwrap();
            return match V::from_bytes(Cow::Owned(r.clone())) {
                Ok(v) => Some(v),
                _ => None,
            };
        }
        // the store
        self.get_store().get(&key)
    }
}

fn get_store_wrap<T: Access>(snap: T, name: &str) -> ProofMapIndex<T::Base, Vec<u8>, Vec<u8>> {
    snap.get_proof_map(name)
}

#[test]
fn test_concat_keys() {
    // Assume keys are always Vec<u8>.
    // We want to prefix with app name.
    let db: Box<dyn Database> = Box::new(TemporaryDB::new());
    let mut c1 = CacheMap::new();

    let snap = db.snapshot();
    let mut store = StoreWrapper::<_, Person>::new(&snap, "hello".into(), &mut c1);

    store.put(
        vec![1],
        Person {
            name: "bob".into(),
            age: 1u8,
        },
    );

    store.put(
        vec![2],
        Person {
            name: "carl".into(),
            age: 2u8,
        },
    );

    let p = store.get(&vec![1]).unwrap();
    assert_eq!("bob", p.name);

    let t = c1.into_cache();
    println!("{:?}", t);

    let c2 = CacheMap::wrap(t);
    println!("{:?}", c2);
}

#[test]
fn test_keyconcat() {
    let mut n = "hello".as_bytes().to_vec();
    let mut x: Vec<u8> = b"what".to_vec();
    n.append(&mut x);
    println!("{:?}", n);
}
