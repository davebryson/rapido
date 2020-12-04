//!
//! Command line application for the Counter Application
//!
//!  Quick use: `cargo run --bin cli create dave`
//!
use borsh::BorshDeserialize;
use rapido_client::{query, send_transaction_commit};
use rapido_core::SignedTransaction;

use counter::{Counter, Msgs, APP_NAME};
use structopt::StructOpt;
use tendermint_rpc::HttpClient;

#[macro_use]
extern crate log;

// CLI Commands
#[derive(StructOpt, Debug)]
#[structopt(about = "Counter App")]
enum CounterAppCommands {
    Create { name: String },
    Add { name: String, value: u16 },
    Subtract { name: String, value: u16 },
    Query { name: String },
}

// Helper: get the tendermint HTTP client
fn get_client() -> HttpClient {
    HttpClient::new("tcp://127.0.0.1:26657".parse().unwrap()).unwrap()
}

// Process the command line
#[tokio::main]
async fn main() {
    let opts = CounterAppCommands::from_args();
    match opts {
        CounterAppCommands::Create { name } => {
            let client = get_client();
            let tx = SignedTransaction::create(name, APP_NAME, Msgs::Create, 0u64);
            match send_transaction_commit(&tx, &client).await {
                Ok(r) => info!("{:?}", r),
                Err(err) => error!("{:?}", err),
            }
        }
        CounterAppCommands::Add { name, value } => {
            let client = get_client();
            let tx = SignedTransaction::create(name, APP_NAME, Msgs::Add(value), 0u64);
            match send_transaction_commit(&tx, &client).await {
                Ok(r) => info!("{:?}", r),
                Err(err) => error!("{:?}", err),
            }
        }
        CounterAppCommands::Subtract { name, value } => {
            let client = get_client();
            let tx = SignedTransaction::create(name, APP_NAME, Msgs::Subtract(value), 0u64);
            match send_transaction_commit(&tx, &client).await {
                Ok(r) => info!("{:?}", r),
                Err(err) => error!("{:?}", err),
            }
        }
        CounterAppCommands::Query { name } => {
            let client = get_client();
            match query(APP_NAME, name.as_bytes().to_vec(), &client).await {
                Ok(count_bits) => {
                    let o = Counter::try_from_slice(&count_bits).unwrap();
                    info!(" {:} => {:?}", name, o)
                }
                Err(err) => error!("{:?}", err),
            }
        }
    }
}
