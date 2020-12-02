use anyhow::bail;
use borsh::{BorshDeserialize, BorshSerialize};

#[macro_use]
extern crate rapido;

use rapido::{AppBuilder, AppModule, Context, SignedTransaction, Store, StoreView, TestKit};

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

    fn handle_tx(&self, ctx: &Context, cache: &mut StoreView) -> Result<(), anyhow::Error> {
        // Use the payload vs msgid
        let msg: Msgs = ctx.decode_msg();
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
        }
    }

    fn handle_query(
        &self,
        path: &str,
        key: Vec<u8>,
        view: &StoreView,
    ) -> Result<Vec<u8>, anyhow::Error> {
        match path {
            "/" => get_person(String::from_utf8(key).unwrap(), view),
            "/random" => query_random(),
            _ => bail!(""),
        }
    }
}

// Queries
fn get_person(key: String, view: &StoreView) -> Result<Vec<u8>, anyhow::Error> {
    let store = MyStore {};
    match store.query(key, view) {
        Some(p) => Ok(p.try_to_vec().unwrap()),
        None => bail!("person not found"),
    }
}

fn query_random() -> Result<Vec<u8>, anyhow::Error> {
    Ok(vec![1])
}

#[test]
fn test_with_testkit() {
    let mut tester = TestKit::create(AppBuilder::new().with_app(PersonHandler {}));
    tester.start();

    let tx1 = SignedTransaction::create(
        "dave",
        "person_app",
        Msgs::CreatePerson("bob".into(), 1),
        0u64,
    );

    assert!(tester.check_tx(&tx1).is_ok());
    assert!(tester.commit_tx(&tx1).is_ok());

    let qr = tester
        .query("person_app", "bob".as_bytes().to_vec())
        .unwrap();
    let p = Person::try_from_slice(&qr[..]).unwrap();
    assert_eq!("bob", p.name);

    assert!(tester
        .query("person_app", "will_fail".as_bytes().to_vec())
        .is_err());
}
