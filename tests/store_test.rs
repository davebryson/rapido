use borsh::{BorshDeserialize, BorshSerialize};
use exonum_merkledb::{BinaryValue, Database, TemporaryDB};
use std::{borrow::Cow, convert::AsRef};

use rapido::{Store, StoreView};

#[macro_use]
extern crate rapido;

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
fn test_store() {
    let db: Box<dyn Database> = Box::new(TemporaryDB::new());
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
