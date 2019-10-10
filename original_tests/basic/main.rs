use abci::*;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{BinaryValue, Database, TemporaryDB};
use rapido::{AccountId, AppBuilder, Node, Service, Transaction, TxResult};
use std::sync::Arc;

use std::borrow::Cow;

mod cryptocurrency;

use cryptocurrency::{
    Account, CreateAcctTx, CryptocurrencyService, DepositTx, TransferTx, CRYPTO_SERVICE_ROUTE_NAME,
};

fn create_app(db: Arc<TemporaryDB>) -> Node {
    AppBuilder::new(db)
        .add_service(Box::new(CryptocurrencyService {}))
        .finish()
}

fn send_and_check_code<M>(app: &mut Node, msgtype: u8, msg: M, expected_code: u32)
where
    M: BorshDeserialize + BorshSerialize,
{
    let mut tx = Transaction::new(CRYPTO_SERVICE_ROUTE_NAME, msgtype, msg);
    let bits = tx.try_to_vec().unwrap();

    let mut req = RequestDeliverTx::new();
    req.set_tx(bits);
    let resp = app.deliver_tx(&req);
    assert_eq!(expected_code, resp.code);
}

fn check_query_account(app: &mut Node, excode: u32, exacct: AccountId, exbal: u64) {
    // Request
    let mut q = RequestQuery::new();
    q.path = format!("{}**anything", CRYPTO_SERVICE_ROUTE_NAME);
    q.data = base64::encode(&exacct).as_bytes().to_vec();
    let resp = app.query(&q);
    assert_eq!(excode, resp.code);

    // Response
    let decode_b64 = base64::decode(&resp.value);
    assert!(decode_b64.is_ok());
    let account = Account::from_bytes(Cow::from(decode_b64.unwrap()));
    assert!(account.is_ok());
    let expected_account = account.unwrap();
    assert_eq!(exacct, expected_account.name);
    assert_eq!(exbal, expected_account.balance);
}

#[test]
fn test_create_account() {
    let db = Arc::new(TemporaryDB::new());
    let mut app = create_app(db.clone());

    let dave = vec![0x11];

    let create_account = CreateAcctTx {
        account: dave.clone(),
    };
    send_and_check_code(&mut app, 0, create_account, 0); // <= bueno

    app.commit(&RequestCommit::new()); /* MUST commit */

    // Duplicate account fails
    let create_account1 = CreateAcctTx {
        account: dave.clone(),
    };

    send_and_check_code(&mut app, 0, create_account1, 12); // <= fail code
    check_query_account(&mut app, 0u32, dave.clone(), 10u64);
}
