use abci::*;
use anyhow::bail;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_merkledb::{BinaryValue, Snapshot, TemporaryDB};
use std::sync::Arc;
use std::{borrow::Cow, convert::AsRef};

#[macro_use]
extern crate rapido;

use rapido::{AppBuilder, AppModule, CacheMap, Context, SignedTransaction, Store};

// Model
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Default)]
pub struct Person {
    name: String,
    age: u8,
}

impl_store_values!(Person);

// Store
pub struct MyStore;
impl Store for MyStore {
    type Key = String;
    type Value = Person;

    fn name(&self) -> String {
        "mystore.person".into()
    }
}

// Messages
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum Msgs {
    CreatePerson(String, u8),
    IncPersonAge(String),
}

// Handler
pub struct PersonHandler;
impl AppModule for PersonHandler {
    fn name(&self) -> &'static str {
        "person_app"
    }

    fn handle_tx(&self, ctx: &Context, cache: &mut CacheMap) -> Result<(), anyhow::Error> {
        // Use the payload vs msgid
        let msg = Msgs::try_from_slice(&ctx.msg)?;
        match msg {
            Msgs::CreatePerson(name, age) => {
                let store = MyStore {};
                store.put(
                    name.clone(),
                    Person {
                        name: name.clone(),
                        age: age,
                    },
                    cache,
                );
                ctx.dispatch_event("person", &[("added", &name.clone())]);
                return Ok(());
            }
            Msgs::IncPersonAge(name) => {
                let store = MyStore {};
                match store.get(name.clone(), cache) {
                    Some(mut p) => {
                        p.age += 1;
                        store.put(name, p, cache);
                        //ctx.dispatch_event("person", &[("aged", &p.name.clone())]);
                        return Ok(());
                    }
                    _ => bail!("person not found"),
                }
            }
            _ => bail!("unknown message"),
        }
    }

    fn handle_query(
        &self,
        path: &str,
        key: Vec<u8>,
        snapshot: &Box<dyn Snapshot>,
    ) -> Result<Vec<u8>, anyhow::Error> {
        match path {
            "/" => get_person(String::from_utf8(key).unwrap(), snapshot),
            "/random" => query_random(),
            _ => bail!(""),
        }
    }
}

fn get_person(key: String, snapshot: &Box<dyn Snapshot>) -> Result<Vec<u8>, anyhow::Error> {
    let store = MyStore {};
    match store.query(key, snapshot) {
        Some(p) => Ok(p.to_bytes()),
        None => bail!("person not found"),
    }
}

// Queries

fn query_random() -> Result<Vec<u8>, anyhow::Error> {
    Ok(vec![1])
}

// Helpers

fn create_person_tx(name: String, age: u8) -> RequestDeliverTx {
    let msg = Msgs::CreatePerson(name, age);
    let tx = SignedTransaction::new([0u8; 10].to_vec(), "person_app", 0u8, msg, 0u64);
    let mut req = RequestDeliverTx::new();
    req.set_tx(tx.encode());
    req
}

fn run_batch(node: &mut rapido::Node, data: Vec<&str>) {
    for name in data {
        let resp = node.deliver_tx(&create_person_tx(name.into(), 20));
        assert_eq!(0u32, resp.code);
        assert_eq!(1, resp.events.len());
        let c1 = node.commit(&RequestCommit::new());
        assert_ne!(resp.data, c1.data);
    }
}

#[test]
fn test_basics() {
    let db = Arc::new(TemporaryDB::new());
    let mut node = AppBuilder::new(db)
        .add_service(Box::new(PersonHandler {}))
        .finish();

    let test_accounts = vec!["a", "b", "c", "d", "e", "f"];

    node.init_chain(&RequestInitChain::new());
    let resp = node.commit(&RequestCommit::new());
    assert!(resp.data.len() > 0);

    let resp = node.deliver_tx(&create_person_tx("dave".into(), 10));
    assert_eq!(0u32, resp.code);
    println!("{:?}", resp.events);
    assert_eq!(1, resp.events.len());
    let c1 = node.commit(&RequestCommit::new());
    assert_ne!(resp.data, c1.data);

    let mut query = RequestQuery::new();
    query.path = "person_app".into();
    query.data = "dave".as_bytes().to_vec();
    let resp = node.query(&query);
    assert_eq!(0u32, resp.code);
    let p = Person::try_from_slice(&resp.value[..]).unwrap();
    assert_eq!("dave", p.name);

    let resp = node.commit(&RequestCommit::new());
    assert!(resp.data.len() > 0);

    run_batch(&mut node, test_accounts);
    node.commit(&RequestCommit::new());

    {
        let mut query = RequestQuery::new();
        query.path = "person_app".into();
        query.data = "dave".as_bytes().to_vec();
        let resp = node.query(&query);
        assert_eq!(0u32, resp.code);
        let p = Person::try_from_slice(&resp.value[..]).unwrap();
        assert_eq!("dave", p.name);
    }

    {
        let mut query = RequestQuery::new();
        query.path = "person_app".into();
        query.data = "c".as_bytes().to_vec();
        let resp = node.query(&query);
        assert_eq!(0u32, resp.code);
        let p = Person::try_from_slice(&resp.value[..]).unwrap();
        assert_eq!("c", p.name);
    }
}
