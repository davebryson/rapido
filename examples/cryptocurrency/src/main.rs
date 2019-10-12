use borsh::BorshSerialize;
use exonum_merkledb::{BinaryValue, TemporaryDB};
use rapido::AppBuilder;
use std::borrow::Cow;
use std::sync::Arc;
use structopt::StructOpt;

use cryptocurrency::{
    Account, CreateAcctTx, CryptocurrencyService, DepositTx, TransferTx, CRYPTO_SERVICE_ROUTE_NAME,
};
use rapido::{AccountId, SignedTransaction};
use rapido_client::RpcClient;

const TMURL: &str = "http://127.0.0.1:26657";

/// Simple CLI to the test the example app. Warning the RpcClient is tricky!!

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

// Map accounts for ease of use
fn get_account(name: &str) -> Option<AccountId> {
    match name {
        "dave" => Some([1u8; 32]),
        "bob" => Some([2u8; 32]),
        "alice" => Some([3u8; 32]),
        "tom" => Some([3u8; 32]),
        _ => None,
    }
}

// **Commands** //

// Run the application node.  This should be ran in a seperate terminal
fn run_app() {
    println!("Running node...");
    let db = Arc::new(TemporaryDB::new());
    let node = AppBuilder::new(db)
        .add_service(Box::new(CryptocurrencyService {}))
        .finish();
    abci::run_local(node);
}

// CLI Command: Send Tx to create a new account
fn create_account(sender: String) {
    let client = RpcClient::new(TMURL);
    let n = get_account(&sender).unwrap();
    let tx = SignedTransaction::new(n, CRYPTO_SERVICE_ROUTE_NAME, 0u16, CreateAcctTx {});
    let encoded = tx.try_to_vec().unwrap();
    let result = client.broadcast_tx_commit(encoded).unwrap();
    println!("create account: {:?}", result);
}

// CLI Command:  Query an account
fn query_account(sender: String) {
    let client = RpcClient::new(TMURL);
    let n = get_account(&sender).unwrap();
    let result = client.abci_query("cryptoapp/", n[..].to_vec()).unwrap();

    // Parse into Json so we can access it
    let packet = json::parse(&result).unwrap();
    println!("{}", packet);

    let st = &packet["response"]["value"].as_str().unwrap();

    // Gotta base64 decode it.  TM automatically wraps in base64
    let v = base64::decode(st).unwrap();
    let acct = Account::from_bytes(Cow::from(v)).unwrap();
    println!("Account Info:");
    println!("  ID : {:?}", acct.account);
    println!("  Bal: {:?}", acct.balance);
}

// CLI Command: Deposit
fn deposit_to_account(sender: String, amt: u64) {
    let client = RpcClient::new(TMURL);
    let n = get_account(&sender).unwrap();
    let tx = SignedTransaction::new(n, CRYPTO_SERVICE_ROUTE_NAME, 1u16, DepositTx(amt));
    let encoded = tx.try_to_vec().unwrap();
    let result = client.broadcast_tx_commit(encoded).unwrap();
    println!("deposit made: {:?}", result);
}

// CLI Command: Xfer
fn transfer_some(sender: String, recip: String, amt: u64) {
    let client = RpcClient::new(TMURL);
    let sender_from = get_account(&sender).unwrap(); // <==
    let recip = get_account(&recip).unwrap();
    let tx = SignedTransaction::new(
        sender_from,
        CRYPTO_SERVICE_ROUTE_NAME,
        2u16,
        TransferTx(recip, amt),
    );
    let encoded = tx.try_to_vec().unwrap();
    let result = client.broadcast_tx_commit(encoded).unwrap();
    println!("transfer made: {:?}", result);
}

fn main() {
    match Commands::from_args() {
        Commands::Create { sender } => create_account(sender),
        Commands::Deposit { sender, amount } => deposit_to_account(sender, amount),
        Commands::Transfer {
            sender,
            recip,
            amount,
        } => transfer_some(sender, recip, amount),
        Commands::Query { sender } => query_account(sender),
        Commands::Run => {
            // **> Run this command in a seperate terminal
            run_app();
        }
    }
}
