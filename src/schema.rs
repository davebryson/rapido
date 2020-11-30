// Store for rapido information
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    BinaryValue,
};

use std::{borrow::Cow, convert::AsRef};

const RAPIDO_CHAIN_STATE: &str = "rapido.app.state";

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Default)]
pub(crate) struct ChainState {
    // Last height
    pub height: i64,
    // Last accumulated application root hash
    pub apphash: Vec<u8>,
}

impl_store_values!(ChainState);

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
