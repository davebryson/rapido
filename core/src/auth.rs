//! Default authenticators you can use for testing and development

use crate::{Authenticator, SignedTransaction, StoreView};

/// Default authenticator used if one is not set in the AppBuilder.
/// Returns Ok for any Tx. and does not increment a nonce.
pub struct DefaultAuthenticator;
impl Authenticator for DefaultAuthenticator {
    fn validate(&self, _tx: &SignedTransaction, _view: &StoreView) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
