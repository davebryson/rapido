use super::AccountStore;
use crate::{verify_tx_signature, AppModule, SignedTransaction, StoreView};
use anyhow::ensure;

/*
pub fn authenticate(tx: &SignedTransaction, view: &mut StoreView) -> Result<(), anyhow::Error> {
    let store = AccountStore::new();
    let wrappedacct = store.get_account(tx.sender(), view);
    ensure!(wrappedacct.is_some(), "Account not found");

    let acct = wrappedacct.unwrap();
    let pk = &acct.pubkey();
    ensure!(pk.is_some(), "Bad publickey");

    // Check the signature
    ensure!(verify_tx_signature(tx, &pk.unwrap()), "Bad signature");

    // Check the nonce
    ensure!(acct.nonce() == tx.nonce(), "nonce doesn't match");

    Ok(())
}
*/
