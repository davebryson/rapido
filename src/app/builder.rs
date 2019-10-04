use exonum_merkledb::Database;
use std::sync::Arc;

use super::traits::TxHandler;
use super::types::ValidateTxHandler;
use super::Node;

/// TODO: move to mod...
pub struct AppBuilder {
    pub db: Arc<dyn Database>,
    pub handlers: Vec<Box<dyn TxHandler>>,
    pub validate_tx_handler: Option<ValidateTxHandler>,
}

impl AppBuilder {
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self {
            db,
            handlers: Vec::new(),
            validate_tx_handler: None,
        }
    }

    pub fn set_validation_handler(mut self, handler: ValidateTxHandler) -> Self {
        self.validate_tx_handler = Some(handler);
        self
    }

    pub fn add_handler(mut self, handler: Box<dyn TxHandler>) -> Self {
        self.handlers.push(handler);
        self
    }

    pub fn finish(self) -> Node {
        Node::new(self)
    }
}
