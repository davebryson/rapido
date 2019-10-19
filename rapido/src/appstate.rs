use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{
    impl_object_hash_for_binary_value, BinaryValue, Entry, ObjectAccess, ObjectHash, ProofMapIndex,
    RefMut,
};
use std::{
    borrow::Cow,
    convert::{AsRef, TryFrom},
};

const APP_STATE_STORE: &str = "_rapido_state_store_";
const APP_ROOT_STORE: &str = "_rapido_proof_table_";

// App state provide blockchain application information.  It's used to determine
// if a given node is in sync.  This information is not included in the overall app hash.
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
pub struct AppState {
    // Last height
    pub height: i64,
    // Last accumulated application root hash
    pub hash: Vec<u8>,
}

impl BinaryValue for AppState {
    fn to_bytes(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        AppState::try_from_slice(bytes.as_ref()).map_err(From::from)
    }
}

impl_object_hash_for_binary_value! { AppState }

/// Composite key for the proof table
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct ProofTableKey(String, u8);
impl ProofTableKey {
    /// Encode a route, service store hash index, into a proof table key. Will error if
    /// `index` is larger then an u8, or the contents cannot be serialized using Borsh
    pub fn encode<R: Into<String>>(route: R, index: usize) -> Result<Vec<u8>, failure::Error> {
        let indexu8 =
            u8::try_from(index).map_err(|_| failure::err_msg("proof table key index not u8:"))?;
        ProofTableKey(route.into(), indexu8)
            .try_to_vec()
            .map_err(|_| failure::err_msg("proof table key encoding:"))
    }

    /// Decode a vec into a proof table key
    pub fn decode(raw: Vec<u8>) -> Result<Self, failure::Error> {
        Self::try_from_slice(&raw[..]).map_err(|_| failure::err_msg("proof table key decoding:"))
    }
}

/// Store for the overall application state.  It uses a non-proof Map to
/// store the last known block height and application root hash.  And it has
/// Merkle Proof Map to store all the individual service root hashes to calculate a
/// single, overall application root hash.
pub struct AppStateStore<T: ObjectAccess>(T);

impl<T: ObjectAccess> AppStateStore<T> {
    /// Get the store given a snapshot or fork
    pub fn new(object_access: T) -> Self {
        Self(object_access)
    }

    /// Return the application state map
    pub fn commit_table(&self) -> RefMut<Entry<T, AppState>> {
        self.0.get_object(APP_STATE_STORE)
    }

    /// Return the application hash proof map
    pub fn proof_table(&self) -> RefMut<ProofMapIndex<T, Vec<u8>, Hash>> {
        self.0.get_object(APP_ROOT_STORE)
    }

    /// Update the lastest commit meta information
    pub fn set_commit_info(&self, height: i64, hash: Vec<u8>) {
        self.commit_table().set(AppState { height, hash });
    }

    /// Get the lastest commit information
    pub fn get_commit_info(&self) -> Option<AppState> {
        self.commit_table().get()
    }

    /// Get the application hash proof map
    pub fn get_proof_table_root_hash(&self) -> Hash {
        self.proof_table().object_hash()
    }

    /// Save the root hash of a specific store in a given service. It uses a composite key
    /// for each service, storing the information in the ProofMap in this format:
    /// ```text
    ///         Key     |  Value
    ///  --------------------------
    ///   ProofTableKey |  Hash
    ///  --------------------------
    /// ```
    /// `index_of_store` is determined by the index of the hash in the Vec returned
    /// from `service.store_hashes()`. For example, if `store_hashes()` returns:
    ///
    /// ```text
    ///   vec![hash1, hash2]
    /// ```
    /// then hash1 = index 0 and hash2 = index 1.
    ///
    /// This method should never need to be called from a service.  It's called
    /// automatically on abci.commit.
    pub fn save_service_hash(&self, service: &str, index: usize, hash: &Hash) {
        let key = ProofTableKey::encode(service, index).expect("save service hash");
        self.proof_table().put(&key, *hash);
    }

    /// Get a root hash from a specific service store. See `save_service_hash`.
    /// This is used on abci query.
    pub fn get_service_hash(&self, key: Vec<u8>) -> Option<Hash> {
        self.proof_table().get(&key)
    }
    
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_proof_table_key_format() {
        assert!(ProofTableKey::encode("hello", 256).is_err());
        assert!(ProofTableKey::encode("hello", 255).is_ok());

        let k1 = ProofTableKey::encode("hello", 255).unwrap();
        assert!(k1.len() > 0);

        let k1back = ProofTableKey::decode(k1).unwrap();
        assert_eq!(255u8, k1back.1);
        assert_eq!("hello", k1back.0);

        assert!(ProofTableKey::decode(vec![]).is_err());
    }
}
