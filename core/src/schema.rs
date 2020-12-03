//! Internal storage
use std::convert::AsRef;

use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    ProofMapIndex,
};

// 2 separate rockdb columns
const RAPIDO_CHAIN_STATE: &str = "rapido.app.state";
const RAPIDO_CORE_MAP: &'static str = "rapido.core.map";

// Holds the chain state information used by Tendermint to sync with the node.
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Default)]
pub(crate) struct ChainState {
    // Last height
    pub height: i64,
    // Last accumulated application root hash
    pub apphash: Vec<u8>,
}

impl_store_values!(ChainState);

// Simple entry storage for chain state that doesn't affect overall state root hash
#[derive(Debug)]
pub(crate) struct RapidoSchema<T: Access> {
    access: T,
}

impl<T: Access> RapidoSchema<T> {
    pub fn new(access: T) -> Self {
        Self { access }
    }

    pub fn get_chain_state(&self) -> Option<ChainState> {
        self.access.get_entry(RAPIDO_CHAIN_STATE).get()
    }
}

impl<T: Access> RapidoSchema<T>
where
    T::Base: RawAccessMut,
{
    pub fn save_chain_state(&mut self, height: i64, apphash: Vec<u8>) {
        self.access
            .get_entry(RAPIDO_CHAIN_STATE)
            .set(ChainState { height, apphash });
    }
}

// Helper to access the app state merkle tree
pub(crate) fn get_store<T: Access>(access: T) -> ProofMapIndex<T::Base, Hash, Vec<u8>> {
    access.get_proof_map(RAPIDO_CORE_MAP)
}
