use exonum_crypto::{Hash, PublicKey};

/// Generate a DID given a PublicKey
/// Format:
///   identifier = base58( sha256(publickey) )
//    did:rapido:{identifier}
pub fn generate_did(pk: PublicKey) -> String {
    let identifer =
        bs58::encode(exonum_crypto::hash(&pk.as_bytes()[..]).as_bytes().to_vec()).into_string();
    format!("did:rapido:{}", identifer)
}

pub struct DidPubKey {
    id: String,
    ktype: String,
    key: Vec<u8>,
}

pub struct DidService {
    id: String,
    stype: String,
    endpoint: String,
}

pub struct DidDocument {
    id: String,
    keys: Vec<DidPubKey>,
    services: Vec<DidService>,
    authenticate: Vec<String>,
}
