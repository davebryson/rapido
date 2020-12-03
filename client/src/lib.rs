//! Simple RPC helper functions to interact with a Tendermint node.
//! ideal for command line applications.
//! Currently supports sending transactions and querying the app.
use std::str::FromStr;

use anyhow::ensure;

use tendermint::abci::Transaction;
use tendermint_rpc::{endpoint::broadcast, Client, HttpClient};

use rapido_core::SignedTransaction;

fn parse_tx_commit_response(resp: broadcast::tx_commit::Response) -> Result<String, anyhow::Error> {
    ensure!(
        resp.check_tx.code.is_ok(),
        "check err: {:}",
        resp.check_tx.log
    );
    ensure!(
        resp.deliver_tx.code.is_ok(),
        "deliver err: {:}",
        resp.check_tx.log
    );

    Ok(format!("success!  tx hash: {:}", resp.hash.to_string()))
}

fn parse_tx_sync_response(resp: broadcast::tx_sync::Response) -> Result<String, anyhow::Error> {
    ensure!(resp.code.is_ok(), resp.log);
    Ok(format!("success!  tx hash: {:}", resp.hash.to_string()))
}

/// Send a tx and wait for its inclusion in a block.  Returns the
/// results of both the check and deliver.
pub async fn send_transaction_commit(
    tx: &SignedTransaction,
    client: &HttpClient,
) -> Result<String, anyhow::Error> {
    let resp = client
        .broadcast_tx_commit(Transaction::from(tx.encode()))
        .await?;
    parse_tx_commit_response(resp)
}

/// Send a transaction and only return the results of the check.
pub async fn send_transaction_sync(
    tx: &SignedTransaction,
    client: &HttpClient,
) -> Result<String, anyhow::Error> {
    let resp = client
        .broadcast_tx_sync(Transaction::from(tx.encode()))
        .await?;
    parse_tx_sync_response(resp)
}

/// Query a particular application (by its registered name). Returns the
/// result as a Vec<u8>.  It's up to the consuming application to determine
/// how to code the value.
pub async fn query(
    app_path: &str,
    key: Vec<u8>,
    client: &HttpClient,
) -> Result<Vec<u8>, anyhow::Error> {
    let p = tendermint::abci::Path::from_str(app_path);
    ensure!(p.is_ok(), "problem parsing app name (path)");
    let resp = client
        .abci_query(Some(p.unwrap()), key, None, false)
        .await?;
    ensure!(resp.code.is_ok(), "query err: {:}", resp.log);
    Ok(resp.value)
}
