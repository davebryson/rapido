use exonum_crypto::gen_keypair;

#[macro_use]
extern crate rapido_core;

use rapido_core::{AppBuilder, SignedTransaction, TestKit};

pub mod app;
use app::{Model, ModelApp, Msgs, TestAuthenticator};

#[test]
fn test_core_basics() {
    // App name
    let appone = "appone";
    // Account
    let account = "dave";

    let mut tester = TestKit::create(AppBuilder::new().with_app(ModelApp::new(appone)));
    tester.start();

    {
        let txs = &[&SignedTransaction::create(
            account,
            appone,
            Msgs::Create(1),
            0u64,
        )];

        assert!(tester.check_tx(txs).is_ok());
        assert!(tester.commit_tx(txs).is_ok());

        let qr = tester.query("appone", account).unwrap();
        let m = Model::decode(qr);
        assert_eq!(1, m.value);
    }

    assert!(tester.query("appone", "badaccountname").is_err());

    {
        let txs = &[&SignedTransaction::create(account, appone, Msgs::Inc, 0u64)];

        assert!(tester.check_tx(txs).is_ok());
        assert!(tester.commit_tx(txs).is_ok());

        let qr = tester.query("appone", account).unwrap();
        let m = Model::decode(qr);
        assert_eq!(2, m.value);
    }
}

#[test]
fn test_multi_mods() {
    // App names
    let app1 = "app1";
    let app2 = "app2";
    let app3 = "app3";

    // Accounts
    let bob = "bob";
    let alice = "alice";

    let app = AppBuilder::new()
        .with_app(ModelApp::new(app1))
        .with_app(ModelApp::new(app2))
        .with_app(ModelApp::new(app3));

    let mut tester = TestKit::create(app);
    tester.start();

    let txs = &[
        &SignedTransaction::create(bob, app1, Msgs::Create(1), 0u64),
        &SignedTransaction::create(alice, app1, Msgs::Create(1), 0u64),
        &SignedTransaction::create(bob, app3, Msgs::Create(1), 0u64),
        &SignedTransaction::create(alice, app3, Msgs::Create(1), 0u64),
        &SignedTransaction::create(bob, app2, Msgs::Create(1), 0u64),
    ];

    assert!(tester.check_tx(txs).is_ok());
    assert!(tester.commit_tx(txs).is_ok());

    assert!(tester.query(app1, bob).is_ok());
    assert!(tester.query(app1, alice).is_ok());
    assert!(tester.query(app2, bob).is_ok());
    assert!(tester.query(app2, bob).is_ok());
    assert!(tester.query(app2, bob).is_ok());
    assert!(tester.query(app2, alice).is_err());
}

#[test]
fn test_check_with_simple_authenticator() {
    let app1 = "app1";
    let alice = "alice";
    let badguy = "bad";

    let (alicepk, alicesk) = gen_keypair();
    let (_badpk, badsk) = gen_keypair();

    let app = AppBuilder::new()
        .set_authenticator(TestAuthenticator::new(alicepk))
        .with_app(ModelApp::new(app1));

    let mut tester = TestKit::create(app);
    tester.start();

    // Check alice passes
    let mut alicetx = SignedTransaction::create(alice, app1, Msgs::Create(1), 0u64);
    alicetx.sign(&alicesk);

    let txs = &[&alicetx];
    assert!(tester.check_tx(txs).is_ok());

    // Check 'bad' fails
    let mut badtx = SignedTransaction::create(badguy, app1, Msgs::Create(1), 0u64);
    badtx.sign(&badsk);

    let txs1 = &[&badtx];
    assert!(tester.check_tx(txs1).is_err());
}
