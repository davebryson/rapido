use super::types::{TxContext, TxResult};
use super::Tx;
use exonum_crypto::Hash;
use exonum_merkledb::Fork;
use protobuf::error::{ProtobufError, ProtobufResult};

/// Implement Service for application logic.
/// Each application may have 1 or more. Each service is
/// keyed by the application by the 'route'. So you
/// should use a unique route name
pub trait Service {
    // The routing name of the service. This cooresponds to
    // the route field in a Tx.
    fn route(&self) -> String;

    // Main entry point to your application. Here's where you
    // implement state transistion logic and interact with storage.
    fn execute(&self, tx: &Tx, fork: &Fork);

    // This function is called on ABCI commit to accumulate a new
    // root hash across all services. You should return the current
    // root hash from you state db.  If you app use more than one form
    // of storage, you should return a hash of all you storage root hashes.
    fn root_hash(&self) -> Hash;
}

// Applications should implement this for STFs
pub trait TxHandler {
    fn route(&self) -> String;
    fn execute(&self, ctx: TxContext) -> TxResult;
}

pub trait IntoProtoBytes<P> {
    /// Encode a Rust struct to Protobuf bytes.
    fn into_proto_bytes(self) -> ProtobufResult<Vec<u8>>;
}

pub trait FromProtoBytes<P>: Sized {
    /// Decode a Rust struct from encoded Protobuf bytes.
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ProtobufError>;
}
