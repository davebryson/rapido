/// Use for values that will be stored in the merkle db.
/// Implements BinaryValue and ObjectHash
/// Note: the given type must derive BorshSerialize/Deserialize
#[macro_export]
macro_rules! impl_store_values {
    ($( $type:ty ),*) => {
        $(
            impl exonum_merkledb::BinaryValue for $type {
                fn to_bytes(&self) -> Vec<u8> {
                    self.try_to_vec().unwrap()
                }

                fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, anyhow::Error> {
                    Self::try_from_slice(bytes.as_ref()).map_err(From::from)
                }
            }

            #[allow(clippy::use_self)] // false positive
            impl exonum_merkledb::ObjectHash for $type {
                fn object_hash(&self) -> exonum_crypto::Hash {
                    exonum_crypto::hash(&self::BinaryValue::to_bytes(self))
                }
            }
        )*
    };
}
