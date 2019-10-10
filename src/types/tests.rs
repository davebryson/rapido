use abci::{ResponseCheckTx, ResponseDeliverTx};
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::gen_keypair;
use exonum_merkledb::{Database, TemporaryDB};
use std::sync::Arc;

use super::{
    schema::{AppState, AppStateSchema},
    verify_tx_signature, Transaction, TxResult,
};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
struct Msg1 {
    pub val: u8,
}

impl Msg1 {
    pub fn new(val: u8) -> Self {
        Self { val }
    }
}

#[test]
fn test_result_conversions() {
    let txok = TxResult::ok();
    let txerr = TxResult::error(11u32, "what!");

    let checkok: ResponseCheckTx = txok.into();
    assert_eq!(0u32, checkok.code);
    assert_eq!(String::from(""), checkok.log);

    let checkerr: ResponseCheckTx = txerr.into();
    assert_eq!(11u32, checkerr.code);
    assert_eq!(String::from("what!"), checkerr.log);

    let deliverok: ResponseDeliverTx = TxResult::ok().into();
    assert_eq!(0u32, deliverok.code);

    let delivererr: ResponseDeliverTx = TxResult::error(12u32, "notagain!").into();
    assert_eq!(12u32, delivererr.code);
    assert_eq!(String::from("notagain!"), delivererr.log);
}

#[test]
fn test_app_state() {
    let db = Arc::new(TemporaryDB::new());
    {
        // New uses default
        let snapshot = db.snapshot();
        let schema = AppStateSchema::new(&snapshot);
        let app_state = schema.app_state().get().unwrap_or_default();
        assert_eq!(0i64, app_state.version);
        assert_eq!(Vec::<u8>::new(), app_state.hash);
    }

    {
        let f = db.fork();
        let schema = AppStateSchema::new(&f);
        schema.app_state().set(AppState {
            version: 1i64,
            hash: vec![1, 1],
        });
        let r = db.merge(f.into_patch());
        assert!(r.is_ok());
    }

    {
        let snapshot = db.snapshot();
        let schema = AppStateSchema::new(&snapshot);
        let app_state = schema.app_state().get().unwrap_or_default();
        assert_eq!(1i64, app_state.version);
        assert_eq!(vec![1, 1], app_state.hash);
    }

    {
        let f = db.fork();
        let schema = AppStateSchema::new(&f);
        schema.app_state().set(AppState {
            version: 2i64,
            hash: vec![2, 2],
        });
        let r = db.merge(f.into_patch());
        assert!(r.is_ok());
    }

    {
        let snapshot = db.snapshot();
        let schema = AppStateSchema::new(&snapshot);
        let app_state = schema.app_state().get().unwrap_or_default();
        assert_eq!(2i64, app_state.version);
        assert_eq!(vec![2, 2], app_state.hash);
    }
}

#[test]
fn test_tx() {
    let acct1 = vec![0x11];
    let (pk, sk) = gen_keypair();
    let (pk2, _sk2) = gen_keypair();

    // TODO: How do I want this API to work/look???
    let mut tx = Transaction::new("hello", 0, Msg1::new(10));
    let rtx = tx.sign(&acct1, &sk);
    assert!(rtx.is_ok());
    let bits = tx.try_to_vec().unwrap();

    let btx = Transaction::try_from_slice(&bits[..]).unwrap();
    assert!(verify_tx_signature(&btx, &pk));
    assert_eq!(false, verify_tx_signature(&btx, &pk2));
    assert_eq!(0u8, btx.msgtype);
    assert_eq!(String::from("hello"), btx.route);

    let msg1 = Msg1::try_from_slice(&btx.msg[..]).unwrap();
    assert_eq!(10u8, msg1.val);
}
