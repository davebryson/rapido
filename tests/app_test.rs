use abci::*;
use exonum_merkledb::TemporaryDB;
use rapido::app::{AppBuilder, IntoProtoBytes, Tx, TxContext, TxHandler, TxResult};
use std::sync::Arc;

pub struct SimpleHandler;

impl TxHandler for SimpleHandler {
    fn route(&self) -> String {
        "simple".into()
    }

    fn execute(&self, ctx: TxContext) -> TxResult {
        let address = vec![1u8; 32];
        let data = ctx.tx.msg.clone();
        ctx.store.put(address, data);

        TxResult::ok()
    }
}

#[test]
fn test_app_basics() {
    let db = Arc::new(TemporaryDB::new());
    let mut app = AppBuilder::new(db)
        .add_handler(Box::new(SimpleHandler {}))
        .finish();

    // CheckTx
    let mut tx = Tx::new();
    tx.route = "simple".into();
    tx.msg = vec![1, 1, 1];
    let bits = tx.into_proto_bytes().unwrap();

    let mut req = RequestCheckTx::new();
    req.set_tx(bits);
    let resp = app.check_tx(&req);
    assert_eq!(0u32, resp.code);

    // Test that runs multiple txs, commits and gets the same root
}
