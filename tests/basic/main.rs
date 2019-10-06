use abci::*;
use exonum_merkledb::{BinaryValue, Database, TemporaryDB};
use rapido::{AppBuilder, IntoProtoBytes, Node, Service, Tx, TxResult};
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

fn send_and_check_code(app: &mut Node, msg: Vec<u8>, expected_code: u32) {
    let mut tx = Tx::new();
    tx.route = CRYPTO_SERVICE_ROUTE_NAME.into();
    tx.msg = msg;
    let bits = tx.into_proto_bytes().unwrap();

    let mut req = RequestDeliverTx::new();
    req.set_tx(bits);
    let resp = app.deliver_tx(&req);
    assert_eq!(expected_code, resp.code);
}

#[test]
fn test_simple() {
    let db = Arc::new(TemporaryDB::new());
    let mut app = create_app(db.clone());

    let mut create_account = CreateAcctTx::new();
    create_account.name = "dave".into();
    let msgbits = create_account.into_proto_bytes().unwrap();
    send_and_check_code(&mut app, msgbits, 0);

    app.commit(&RequestCommit::new()); /* MUST commit */

    // Duplicate account fails
    let mut create_account1 = CreateAcctTx::new();
    create_account1.name = "dave".into();
    let msgbits1 = create_account1.into_proto_bytes().unwrap();
    send_and_check_code(&mut app, msgbits1, 12);

    // Try query

    // Client request
    let mut q = RequestQuery::new();
    q.path = format!("{}**anything", CRYPTO_SERVICE_ROUTE_NAME);
    q.data = base64::encode(b"dave").as_bytes().to_vec();
    let resp = app.query(&q);
    assert_eq!(0u32, resp.code);

    // Client decode
    let decode_b64 = base64::decode(&resp.value);
    assert!(decode_b64.is_ok());
    let account = Account::from_bytes(Cow::from(decode_b64.unwrap()));
    assert!(account.is_ok());
    let daves_account = account.unwrap();
    assert_eq!(String::from("dave"), daves_account.name);
    assert_eq!(0u64, daves_account.balance);
}
