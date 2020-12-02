use std::borrow::Cow;
use std::collections::HashMap;

use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{BinaryValue, Fork, Snapshot};

use crate::schema;

/// A change to the store
#[derive(Debug)]
pub enum ViewChange {
    Add(Vec<u8>),
    Remove,
}

impl ViewChange {
    /// Extract the value
    pub fn get(&self) -> Option<&Vec<u8>> {
        match self {
            ViewChange::Add(v) => Some(&v),
            ViewChange::Remove => None,
        }
    }
}

/// Hashmap cache
pub(crate) type Cache = HashMap<Hash, ViewChange>;

/// StoreKey used to prefix each key based on the store.name()
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Default)]
pub(crate) struct StoreKey<K> {
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

/// Provides cached access to a store
#[derive(Debug)]
pub struct StoreView<'a> {
    cache: Cache,
    access: &'a Box<dyn Snapshot>,
}

impl<'a> StoreView<'a> {
    /// Return a new view with cache
    pub fn wrap(db: &'a Box<dyn Snapshot>, cache: Cache) -> Self {
        StoreView {
            access: db,
            cache: cache,
        }
    }

    /// Return a view when we only want the latest snapshot
    pub fn wrap_snapshot(db: &'a Box<dyn Snapshot>) -> Self {
        StoreView {
            access: db,
            cache: Default::default(),
        }
    }

    /// Consume the cache
    pub fn into_cache(self) -> Cache {
        self.cache
    }

    pub fn exists(&self, key: &Hash) -> bool {
        self.cache.contains_key(&key)
    }

    pub fn get(&self, key: &Hash) -> Option<&Vec<u8>> {
        if let Some(cv) = self.cache.get(&key) {
            return cv.get();
        }
        None
    }

    pub fn get_from_store(&self, key: &Hash) -> Option<Vec<u8>> {
        schema::get_store(self.access).get(&key)
    }

    pub fn put(&mut self, key: Hash, value: impl BinaryValue) {
        self.cache.insert(key, ViewChange::Add(value.to_bytes()));
    }

    pub fn remove(&mut self, key: Hash) {
        self.cache.insert(key, ViewChange::Remove);
    }

    /// Called on abci.commit to write all changes to the merkle store
    pub fn commit(&self, fork: &Fork) {
        let mut store = schema::get_store(fork);
        for (k, cv) in &self.cache {
            match cv {
                ViewChange::Add(value) => store.put(k, value.to_owned()),
                ViewChange::Remove => store.remove(k),
            }
        }
    }
}

/// Implement this trait to create a store for your application.
/// An application can have many different stores.
/// Example:
///
pub trait Store: Sync + Send {
    /// Specify the key used for this store.
    /// A key can be any value that fulfills the Borsh se/de traits.
    type Key: BorshSerialize + BorshDeserialize;

    /// Specify what will be stored.  The value must fulfill the
    /// BinaryValue trait.  Use the macro: `impl_store_values()` to do so.
    type Value: BinaryValue;

    /// Return a unique name for the store.  Recommend using  'appname + name'.
    /// For example, if the appname is 'example' and you define a store for 'People'
    /// values, name should return: 'example.people'.  This value must be unique as
    // it's used as a prefix to the key name in the MerkleTree.
    fn name(&self) -> String;

    /// Put a value in the store
    fn put(&self, key: Self::Key, v: Self::Value, view: &mut StoreView) {
        let hash = StoreKey::create(self.name(), key).hash();
        view.put(hash, v)
    }

    /// Get a value from the store
    fn get(&self, key: Self::Key, view: &StoreView) -> Option<Self::Value> {
        let hash = StoreKey::create(self.name(), key).hash();

        // Check the cache first
        if let Some(v) = view.get(&hash) {
            return match Self::Value::from_bytes(Cow::Owned(v.clone())) {
                Ok(r) => Some(r),
                _ => None,
            };
        }

        // Not in the cache, check the latest snapshot of committed values
        if let Some(v) = view.get_from_store(&hash) {
            return match Self::Value::from_bytes(Cow::Owned(v.clone())) {
                Ok(r) => Some(r),
                _ => None,
            };
        }

        None
    }

    /// Query the latest committed data for the value
    fn query(&self, key: Self::Key, view: &StoreView) -> Option<Self::Value> {
        let hash = StoreKey::create(self.name(), key).hash();
        if let Some(v) = view.get_from_store(&hash) {
            return match Self::Value::from_bytes(Cow::Owned(v.clone())) {
                Ok(r) => Some(r),
                _ => None,
            };
        }
        None
    }

    /// Remove a value
    fn remove(&self, key: Self::Key, view: &mut StoreView) {
        let hash = StoreKey::create(self.name(), key).hash();
        view.remove(hash)
    }

    /// Does the give key exists?
    fn contains_key(&self, key: Self::Key, view: &StoreView) -> bool {
        let hash = StoreKey::create(self.name(), key).hash();
        view.exists(&hash)
    }

    // TODO:
    //fn get_proof(&self, key: &Vec<u8>);
}

mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Default)]
    pub struct Person {
        name: String,
        age: u8,
    }

    impl_store_values!(Person);

    // Add BinaryKey as a type also?
    pub struct MyStore;
    impl Store for MyStore {
        type Key = String;
        type Value = Person;

        fn name(&self) -> String {
            "mystore".into()
        }
    }

    #[test]
    fn test_store_basic() {
        let db: Box<dyn exonum_merkledb::Database> = Box::new(exonum_merkledb::TemporaryDB::new());
        let snap = db.snapshot();
        let mut c1 = StoreView::wrap(&snap, Default::default());

        let store = MyStore {};
        store.put(
            "bob".into(),
            Person {
                name: "bob".into(),
                age: 1u8,
            },
            &mut c1,
        );

        store.put(
            "carl".into(),
            Person {
                name: "carl".into(),
                age: 2u8,
            },
            &mut c1,
        );

        // This passes because we haven't committed
        assert!(store.get("bob".into(), &mut c1).is_some());
        assert!(store.get("bad".into(), &mut c1).is_none());

        let t = c1.into_cache();
        println!("{:?}", t);
    }
}
