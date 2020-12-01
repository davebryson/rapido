//! Counter application that's simple and demonstrates Rapido functionality.
//! User's can maintain a counter in a state store.
//! A user account (tx.sender) is just a 'name'.  So:
//! name => Counter{}
//! User's can increase and decrease their Counters and check the current count.
//! For demo purposes, Txs don't need to be signed.

use borsh::{BorshDeserialize, BorshSerialize};
use rapido::{
    client::{query, send_transaction_commit},
    SignedTransaction,
};
use structopt::StructOpt;
use tendermint_rpc::HttpClient;

const APP_NAME: &'static str = "counter.app";

/// Model for the counter.  This is stored in the Merkle Tree
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Default)]
pub struct Counter {
    count: u16,
}

impl Counter {
    pub fn add(&self, value: u16) -> Self {
        Self {
            count: self.count + value,
        }
    }

    pub fn subtract(&self, value: u16) -> Self {
        Self {
            count: self.count - value,
        }
    }

    pub fn to_hex(&self) -> String {
        format!("{:x}", self.count)
    }
}
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum Msgs {
    Create(String),
    Add(u16),
    Subtract(u16),
}

// Create, Add, Sub, query
// run app
// Client CLI
#[derive(StructOpt, Debug)]
#[structopt(about = "Counter App")]
enum CounterAppCommands {
    Create { name: String },
    Add { name: String, value: u16 },
    Subtract { name: String, value: u16 },
    Query { name: String },
}

fn get_client() -> HttpClient {
    HttpClient::new("tcp://127.0.0.1:26657".parse().unwrap()).unwrap()
}

// run: cargo run --example counter create dave
#[tokio::main]
async fn main() {
    let opts = CounterAppCommands::from_args();
    match opts {
        CounterAppCommands::Create { name } => {
            let client = get_client();
            let tx = SignedTransaction::create("", APP_NAME, Msgs::Create(name), 0u64);
            match send_transaction_commit(&tx, &client).await {
                Ok(r) => println!("{:?}", r),
                Err(err) => println!("{:?}", err),
            }
        }
        CounterAppCommands::Add { name, value } => {
            let client = get_client();
            let tx = SignedTransaction::create(name, APP_NAME, Msgs::Add(value), 0u64);
            match send_transaction_commit(&tx, &client).await {
                Ok(r) => println!("{:?}", r),
                Err(err) => println!("{:?}", err),
            }
        }
        CounterAppCommands::Subtract { name, value } => {
            let client = get_client();
            let tx = SignedTransaction::create(name, APP_NAME, Msgs::Subtract(value), 0u64);
            match send_transaction_commit(&tx, &client).await {
                Ok(r) => println!("{:?}", r),
                Err(err) => println!("{:?}", err),
            }
        }
        CounterAppCommands::Query { name } => {
            let client = get_client();
            match query(APP_NAME, name.as_bytes().to_vec(), &client).await {
                Ok(count_bits) => {
                    let o = Counter::try_from_slice(&count_bits).unwrap();
                    println!(" {:} => {:?}", name, o)
                }
                Err(err) => println!("{:?}", err),
            }
        }
    }
}
