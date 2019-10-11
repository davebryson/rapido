///! json-rpc requests for ABCI.
///! The Simplest Possible Thing That'll Work!
use json::{array, object, JsonValue};

/// Implement to generate each json-rpc request
pub trait AbciRequest {
    // return the method name
    fn method(&self) -> &'static str;

    // return the params field
    fn params(&self) -> JsonValue {
        array![]
    }

    // Generate the json
    fn to_json(&self) -> String {
        let req = object! {
            "jsonrpc" => "2.0",
            "method" => self.method(),
            "id" => 1, // <= probably should change this later
            "params" => self.params(),
        };
        req.dump()
    }
}

/// Basic Information
pub struct AbciInfo;
impl AbciRequest for AbciInfo {
    fn method(&self) -> &'static str {
        "abci_info"
    }
}

pub struct NetInfo;
impl AbciRequest for NetInfo {
    fn method(&self) -> &'static str {
        "net_info"
    }
}

/// Send a tx and wait for commit
pub struct BroadcastTxCommit(pub Vec<u8>);
impl AbciRequest for BroadcastTxCommit {
    fn method(&self) -> &'static str {
        "broadcast_tx_commit"
    }

    fn params(&self) -> JsonValue {
        let encoded = base64::encode(&self.0);
        object! {
            "tx" => encoded,
        }
    }
}

pub struct AbciQuery {
    path: String,
    data: Vec<u8>,
    proof: bool,
}
impl AbciQuery {
    pub fn new(path: String, data: Vec<u8>) -> Self {
        Self {
            path,
            data,
            proof: false,
        }
    }
}
impl AbciRequest for AbciQuery {
    fn method(&self) -> &'static str {
        "abci_query"
    }

    fn params(&self) -> JsonValue {
        // TM expects the data(key) to be hex encoded
        let encoded = hex::encode(self.data.clone());
        object! {
            "path" => self.path.clone(),
            "data" => encoded,
            "proof" => self.proof,
        }
    }
}
