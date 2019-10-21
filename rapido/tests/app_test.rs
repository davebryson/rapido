mod services;

use abci::*;
use borsh::BorshSerialize;
use exonum_crypto::SecretKey;
use exonum_merkledb::TemporaryDB;
use rapido::{sign_transaction, AppBuilder, SignedTransaction};
use std::sync::Arc;

use services::{
    accounts::{authenticate_sender, Account, AccountService, ACCOUNT_SERVICE_ROUTE},
    proofodna::{DNAService, RegisterTx, PODNA_SERVICE_ROUTE},
    TestWallet,
};

fn create_tx(sender: Vec<u8>, sk: &SecretKey, msg: RegisterTx) -> Vec<u8> {
    let mut tx = SignedTransaction::new(sender, PODNA_SERVICE_ROUTE, 0, msg);
    sign_transaction(&mut tx, sk);
    let encoded = tx.try_to_vec();
    assert!(encoded.is_ok());
    encoded.unwrap()
}

#[test]
fn test_check_tx() {
    const DNA_STORE_OWNER: usize = 0;
    const BOB: usize = 1;

    let wallet = TestWallet::generate(3);
    let db = Arc::new(TemporaryDB::new());
    let mut node = AppBuilder::new(db)
        .set_validation_handler(authenticate_sender)
        .set_genesis_data(wallet.generate_genesis_data())
        .add_service(Box::new(AccountService {}))
        .add_service(Box::new(DNAService {}))
        .finish();

    node.init_chain(&RequestInitChain::new());
    node.commit(&RequestCommit::new());

    {
        // Fails with account verification error
        let badsender = SignedTransaction::new(
            vec![1u8; 4],
            PODNA_SERVICE_ROUTE,
            0,
            RegisterTx::new(vec![1u8; 4], [2u8; 32]),
        );
        let encoded = badsender.try_to_vec();
        assert!(encoded.is_ok());
        let bits = encoded.unwrap();

        let mut req = RequestCheckTx::new();
        req.set_tx(bits.clone());
        assert_eq!(10u32, node.check_tx(&req).code);
    }

    {
        // Fails: bad signature
        let mut tx = SignedTransaction::new(
            wallet.get_address(BOB),
            PODNA_SERVICE_ROUTE,
            0,
            RegisterTx::new(wallet.get_address(DNA_STORE_OWNER), [2u8; 32]),
        );
        sign_transaction(&mut tx, &wallet.get_secretkey(BOB));
        // ALTER THE SIGNATURE
        tx.signature[0] = tx.signature[0] >> 1;
        let encoded = tx.try_to_vec();
        assert!(encoded.is_ok());
        let bits = encoded.unwrap();

        let mut req = RequestCheckTx::new();
        req.set_tx(bits.clone());
        assert_eq!(10u32, node.check_tx(&req).code);
    }

    {
        // Pass
        let goodtxbits = create_tx(
            wallet.get_address(BOB),
            &wallet.get_secretkey(BOB),
            RegisterTx::new(wallet.get_address(DNA_STORE_OWNER), [2u8; 32]),
        );
        let mut req = RequestCheckTx::new();
        req.set_tx(goodtxbits.clone());
        assert_eq!(0u32, node.check_tx(&req).code);
    }
}

#[test]
fn test_state_transistions() {
    const DNA_STORE_OWNER: usize = 0;
    const BOB: usize = 1;
    const ALICE: usize = 2;

    let wallet = TestWallet::generate(3);
    let db = Arc::new(TemporaryDB::new());
    let mut node = AppBuilder::new(db)
        .set_validation_handler(authenticate_sender)
        .set_genesis_data(wallet.generate_genesis_data())
        .add_service(Box::new(AccountService {}))
        .add_service(Box::new(DNAService {}))
        .finish();

    node.init_chain(&RequestInitChain::new());
    let initial_app_hash = node.commit(&RequestCommit::new());

    {
        // Bob registers his DNA
        let goodtxbits = create_tx(
            wallet.get_address(BOB),
            &wallet.get_secretkey(BOB),
            RegisterTx::new(wallet.get_address(DNA_STORE_OWNER), [2u8; 32]),
        );
        let mut req = RequestDeliverTx::new();
        req.set_tx(goodtxbits.clone());
        assert_eq!(0u32, node.deliver_tx(&req).code);
    }

    let root1 = node.commit(&RequestCommit::new());
    assert_ne!(initial_app_hash, root1);

    {
        // Check Bob's balance
        let mut query = RequestQuery::new();
        query.path = format!("{}/account", ACCOUNT_SERVICE_ROUTE);
        query.data = wallet.get_address(BOB);
        let resp = node.query(&query);
        assert_eq!(0u32, resp.code);
        assert_eq!(0, Account::decode(resp.value).balance)
    }

    {
        // Check DNA Store owner's balance
        let mut query = RequestQuery::new();
        query.path = format!("{}/account", ACCOUNT_SERVICE_ROUTE);
        query.data = wallet.get_address(DNA_STORE_OWNER);
        let resp = node.query(&query);
        assert_eq!(0u32, resp.code);
        assert_eq!(10, Account::decode(resp.value).balance)
    }

    {
        // Alice try to register Bob's DNA
        let txbits = create_tx(
            wallet.get_address(ALICE),
            &wallet.get_secretkey(ALICE),
            RegisterTx::new(wallet.get_address(DNA_STORE_OWNER), [2u8; 32]),
        );
        let mut req = RequestDeliverTx::new();
        req.set_tx(txbits.clone());
        assert!(node.deliver_tx(&req).code != 0);
        node.commit(&RequestCommit::new());
    }

    let root2 = node.commit(&RequestCommit::new());
    assert_eq!(root1, root2); // apphash shouldn't change

    {
        // Now Alice gets it right
        let txbits = create_tx(
            wallet.get_address(ALICE),
            &wallet.get_secretkey(ALICE),
            RegisterTx::new(wallet.get_address(DNA_STORE_OWNER), [3u8; 32]),
        );
        let mut req = RequestDeliverTx::new();
        req.set_tx(txbits.clone());
        assert!(node.deliver_tx(&req).code == 0);
        node.commit(&RequestCommit::new());
    }

    let root3 = node.commit(&RequestCommit::new());
    assert_ne!(root3, root2);

    {
        // Check Alice's balance
        let mut query = RequestQuery::new();
        query.path = format!("{}/account", ACCOUNT_SERVICE_ROUTE);
        query.data = wallet.get_address(ALICE);
        let resp = node.query(&query);
        assert_eq!(0u32, resp.code);
        assert_eq!(0, Account::decode(resp.value).balance)
    }

    {
        // Check DNA Store owner's balance
        let mut query = RequestQuery::new();
        query.path = format!("{}/account", ACCOUNT_SERVICE_ROUTE);
        query.data = wallet.get_address(DNA_STORE_OWNER);
        let resp = node.query(&query);
        assert_eq!(0u32, resp.code);
        assert_eq!(15, Account::decode(resp.value).balance)
    }

    {
        // Verify Bob's DNA
        let mut query = RequestQuery::new();
        query.path = format!("{}/dna", PODNA_SERVICE_ROUTE);
        query.data = wallet.get_address(BOB);
        let resp = node.query(&query);
        assert_eq!(0u32, resp.code);
        assert_eq!(vec![2u8; 32], resp.value);
    }

    {
        // Verify Alice's DNA
        let mut query = RequestQuery::new();
        query.path = format!("{}/dna", PODNA_SERVICE_ROUTE);
        query.data = wallet.get_address(ALICE);
        let resp = node.query(&query);
        assert_eq!(0u32, resp.code);
        assert_eq!(vec![3u8; 32], resp.value);
    }
}
