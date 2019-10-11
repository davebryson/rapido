use borsh::{BorshDeserialize, BorshSerialize};
use exonum_merkledb::{BinaryValue, TemporaryDB};
use rapido::AppBuilder;
use std::sync::Arc;
use std::{borrow::Cow, convert::AsRef};
use structopt::StructOpt;

use cryptocurrency::{Account, CreateAcctTx, CryptocurrencyService, CRYPTO_SERVICE_ROUTE_NAME};
use rapido::{AccountId, SignedTransaction};
use rapido_client::RpcClient;

const TMURL: &str = "http://127.0.0.1:26657";

#[derive(StructOpt, Debug)]
#[structopt(about = "cryptocurrency client")]
pub enum Commands {
    Run,
    Create {
        // Name of the account to create
        sender: String,
    },
    Deposit {
        sender: String,
        amount: u64,
    },
    Transfer {
        sender: String,
        recip: String,
        amount: u64,
    },
    Query {
        sender: String,
    },
}

fn get_account(name: &str) -> Option<AccountId> {
    match name {
        "dave" => Some([1u8; 32]),
        "bob" => Some([2u8; 32]),
        "alice" => Some([3u8; 32]),
        "tom" => Some([3u8; 32]),
        _ => None,
    }
}

// Commands
fn run_app() {
    println!("Running node...");
    let db = Arc::new(TemporaryDB::new());
    let node = AppBuilder::new(db)
        .add_service(Box::new(CryptocurrencyService {}))
        .finish();
    abci::run_local(node);
}

fn create_account(sender: String) {
    let client = RpcClient::new(TMURL);
    let n = get_account(&sender).unwrap();
    let tx = SignedTransaction::new(n, CRYPTO_SERVICE_ROUTE_NAME, 0u16, CreateAcctTx {});
    let encoded = tx.try_to_vec().unwrap();
    let result = client.broadcast_tx_commit(encoded).unwrap();
    println!("create account: {:?}", result);
}

fn query_account(sender: String) {
    let client = RpcClient::new(TMURL);
    let n = get_account(&sender).unwrap();
    let result = client.abci_query("cryptoapp**ex", n[..].to_vec()).unwrap();

    // Parse into Json so we can access it
    let packet = json::parse(&result).unwrap();
    let st = &packet["response"]["value"].as_str().unwrap();

    // Gotta base64 decode it.  I do this in service.query
    let v = base64::decode(st).unwrap();
    let acct = Account::from_bytes(Cow::from(v)).unwrap();
    println!("Account Info:");
    println!("  ID : {:?}", acct.account);
    println!("  Bal: {:?}", acct.balance);
}

fn main() {
    match Commands::from_args() {
        Commands::Create { sender } => create_account(sender),
        Commands::Deposit { sender, amount } => println!("Deposit {}", amount),
        Commands::Transfer {
            sender,
            recip,
            amount,
        } => println!("Transfer from {} to {} amt {}", sender, recip, amount),
        Commands::Query { sender } => query_account(sender),
        Commands::Run => {
            // Run this command in a seperate terminal
            run_app();
        }
    }
}
