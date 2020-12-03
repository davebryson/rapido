//! TestKit is a simple tool to test your Application without running a Tendermint node.
use crate::{AppBuilder, Node, SignedTransaction};
use abci::*;
use anyhow::{bail, ensure};
use exonum_crypto::{hash, PublicKey, SecretKey, Seed};

/// TestKit for testing an application without running Tendermint.
pub struct TestKit {
    node: Node,
    has_init: bool,
}

impl TestKit {
    /// Create the kit from the AppBuilder
    pub fn create(builder: AppBuilder) -> Self {
        Self {
            node: builder.node(),
            has_init: false,
        }
    }

    /// Must call start first and only once.  This simulates Tendermint's
    /// call to initialize genesis data in the application state store.
    pub fn start(&mut self) {
        self.node.init_chain(&RequestInitChain::new());
        self.has_init = true;
    }

    /// Run transactions through the authentication handler. This simulates
    /// how Tendermint checks transactions for inclusion in the mempool.
    pub fn check_tx(&mut self, txs: &[&SignedTransaction]) -> anyhow::Result<(), anyhow::Error> {
        ensure!(self.has_init, "Must first call the start method");

        for tx in txs {
            let mut req = RequestCheckTx::new();
            req.set_tx(tx.encode());
            let resp = self.node.check_tx(&req);

            if resp.code != 0 {
                bail!("reason: {:}", resp.log);
            }
        }
        Ok(())
    }

    /// Run transactions and commit to state if there are no failures. Will return the updated
    /// application state hash used as part of the consensus process in Tendermint.
    pub fn commit_tx(
        &mut self,
        txs: &[&SignedTransaction],
    ) -> anyhow::Result<Vec<u8>, anyhow::Error> {
        ensure!(self.has_init, "Must first call the start method");

        for tx in txs {
            let mut req = RequestDeliverTx::new();
            req.set_tx(tx.encode());
            let resp = self.node.deliver_tx(&req);

            if resp.code != 0 {
                bail!("reason: {:}", resp.log);
            }
        }

        // Commit and return the new apphash
        let commit_resp = self.node.commit(&RequestCommit::new());
        Ok(commit_resp.data)
    }

    /// Query the latest committed state of an application.
    /// Where `path` and `key` are based on the parameters used
    /// in the applications `handle_query` method.
    /// On success, it'll return the response value as bytes.  It's
    /// up to the consumer to decode.
    pub fn query<K: Into<Vec<u8>>>(
        &mut self,
        path: &str,
        key: K,
    ) -> anyhow::Result<Vec<u8>, anyhow::Error> {
        ensure!(self.has_init, "Must first call the start method");

        let mut query = RequestQuery::new();
        query.path = path.into();
        query.data = key.into();
        let resp = self.node.query(&query);

        if resp.code != 0 {
            bail!("reason: {:}", resp.log);
        }
        // return the query value
        Ok(resp.value)
    }
}

pub fn testing_keypair(val: &str) -> (PublicKey, SecretKey) {
    let seed = Seed::new(hash(val.as_bytes()).as_bytes());
    exonum_crypto::gen_keypair_from_seed(&seed)
}

mod tests {
    use super::testing_keypair;

    #[test]
    fn test_kit_wallet() {
        let (apk, ask) = testing_keypair("dave");
        let (bpk, bsk) = testing_keypair("dave");

        assert_eq!(apk, bpk);
        assert_eq!(ask, bsk);
    }
}
