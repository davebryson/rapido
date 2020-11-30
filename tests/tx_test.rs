use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::gen_keypair;

use rapido::{verify_tx_signature, SignedTransaction};

#[derive(BorshDeserialize, BorshSerialize, PartialEq, Debug)]
enum Message {
    Add(u16),
    Send(String),
}

#[test]
fn test_signed_transaction() {
    let accountid = vec![1];
    let (pk, sk) = gen_keypair();
    let mut tx = SignedTransaction::create(accountid.clone(), "example", Message::Add(10u16), 1u64);
    tx.sign(&sk);
    let encoded = tx.encode();

    let back = SignedTransaction::decode(&encoded).unwrap();
    assert!(verify_tx_signature(&back, &pk));

    let ctx = back.into_context();
    assert_eq!(Message::Add(10u16), ctx.decode_msg());
    assert_eq!(accountid, ctx.sender);
    assert_eq!("example", back.appname());
}
