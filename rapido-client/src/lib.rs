use json::parse;
use url::Url;

mod requests;
use requests::{AbciInfo, AbciQuery, AbciRequest, BroadcastTxCommit, NetInfo};

/// Hacky Tendermint RPC Client.   Works with pure Json via the rust-json
/// crate. Each method returns `Result<String, String>` where the value is
/// either the json-rpc payload for 'result' or 'error'. You can then use
/// the value to print or parse for indexed access into the json object.
///
/// *** This should not be used for anything other than testing, demos...***  
pub struct RpcClient {
    url: Url,
}

impl RpcClient {
    pub fn new(url: &str) -> Self {
        Self {
            url: Url::parse(url).expect("bad URL!"),
        }
    }

    pub fn abci_info(&self) -> Result<String, String> {
        self.execute(&AbciInfo {})
    }

    pub fn net_info(&self) -> Result<String, String> {
        self.execute(&NetInfo {})
    }

    pub fn abci_query<P: Into<String>>(&self, path: P, data: Vec<u8>) -> Result<String, String> {
        self.execute(&AbciQuery::new(path.into(), data))
    }

    pub fn broadcast_tx_commit(&self, tx: Vec<u8>) -> Result<String, String> {
        self.execute(&BroadcastTxCommit(tx))
    }

    // Returns either the json String for result or error.
    fn execute(&self, req: &dyn AbciRequest) -> Result<String, String> {
        let host = self.url.host().unwrap();
        let port = self.url.port().unwrap();

        let client = reqwest::Client::new();
        let mut resp = client
            .post(&format!("http://{}:{}/", host, port))
            .header("content-type", "application/json")
            .header("user-agent", "cheapo rapido client")
            .body(req.to_json())
            .send()
            .unwrap();

        let mut response_body = Vec::new();
        resp.copy_to(&mut response_body).unwrap();

        let value = &String::from_utf8(response_body).unwrap();
        let packet = parse(value).unwrap();
        if packet.has_key("error") {
            return Err(packet["error"].pretty(1));
        }
        Ok(packet["result"].pretty(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
        let client = RpcClient::new("http://uipnode1.mitre.org:26657");
        println!("{}", client.net_info().unwrap())
    }
}
