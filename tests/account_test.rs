use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::gen_keypair;
use rapido::{
    account::{dev::generate_dev_accounts, Account},
    SignedTransaction,
};

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum Msgs {
    One,
}

#[test]
fn test_account() {
    let (pk, sk) = gen_keypair();
    let acct = Account::create(pk);
    let accounts = generate_dev_accounts();

    let mut tx = SignedTransaction::create(acct.id(), "test", Msgs::One, 0u64);
    tx.sign(&sk);
}
