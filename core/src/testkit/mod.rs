//! TestKit is a simple tool to test your Application without running a Tendermint node.
use crate::{AppBuilder, Node, SignedTransaction};
use abci::*;
use anyhow::{bail, ensure};

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

    /// Run a transaction through the authentication handler. This simulates
    /// how Tendermint checks transactions for inclusion in the mempool.
    pub fn check_tx(&mut self, tx: &SignedTransaction) -> anyhow::Result<(), anyhow::Error> {
        ensure!(self.has_init, "Must first call the start method");
        let mut req = RequestCheckTx::new();
        req.set_tx(tx.encode());
        let resp = self.node.check_tx(&req);

        if resp.code != 0 {
            bail!("reason: {:}", resp.log);
        }
        Ok(())
    }

    /// Run a transaction and commit to state if it doesn't fail. Will return the updated
    /// application state hash used as part of the consensus process in Tendermint.
    pub fn commit_tx(&mut self, tx: &SignedTransaction) -> anyhow::Result<Vec<u8>, anyhow::Error> {
        ensure!(self.has_init, "Must first call the start method");

        let mut req = RequestDeliverTx::new();
        req.set_tx(tx.encode());
        let resp = self.node.deliver_tx(&req);

        if resp.code != 0 {
            bail!("reason: {:}", resp.log);
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
    pub fn query(&mut self, path: &str, key: Vec<u8>) -> anyhow::Result<Vec<u8>, anyhow::Error> {
        ensure!(self.has_init, "Must first call the start method");

        let mut query = RequestQuery::new();
        query.path = path.into();
        query.data = key;
        let resp = self.node.query(&query);

        if resp.code != 0 {
            bail!("reason: {:}", resp.log);
        }
        // return the query value
        Ok(resp.value)
    }
}
