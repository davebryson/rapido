use abci::{ResponseCheckTx, ResponseDeliverTx};
use protobuf::error::{ProtobufError, ProtobufResult};
use protobuf::Message;

use super::traits::{FromProtoBytes, IntoProtoBytes};
use super::tx::Tx;
use crate::store::StateStore;

pub type ValidateTxHandler = fn(ctx: TxContext) -> TxResult;

/// Results of a executing a Transaction. Returned from checkTx/deliverTx
pub struct TxResult {
    pub code: u32,
    pub log: String,
}

impl TxResult {
    pub fn new(code: u32, log: String) -> Self {
        Self { code, log }
    }

    pub fn ok() -> Self {
        Self {
            code: 0,
            log: "".to_string(),
        }
    }

    pub fn error(code: u32, reason: String) -> Self {
        Self { code, log: reason }
    }
}

/// Convert a TxResult into a abci.checkTx response
impl Into<ResponseCheckTx> for TxResult {
    fn into(self) -> ResponseCheckTx {
        let mut resp = ResponseCheckTx::new();
        resp.set_code(self.code);
        resp.set_log(self.log);
        resp
    }
}

/// Convert a TxResult into a abci.deliverTx response
impl Into<ResponseDeliverTx> for TxResult {
    fn into(self) -> ResponseDeliverTx {
        let mut resp = ResponseDeliverTx::new();
        resp.set_code(self.code);
        resp.set_log(self.log);
        resp
    }
}

pub struct TxContext<'a> {
    pub store: &'a mut StateStore,
    pub tx: &'a Tx,
}

impl<'a> TxContext<'a> {
    pub fn new(store: &'a mut StateStore, tx: &'a Tx) -> Self {
        Self { store, tx }
    }
}

impl IntoProtoBytes<Tx> for Tx {
    fn into_proto_bytes(self) -> ProtobufResult<Vec<u8>> {
        self.write_to_bytes()
    }
}

impl FromProtoBytes<Tx> for Tx {
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ProtobufError> {
        protobuf::parse_from_bytes::<Self>(bytes)
    }
}
