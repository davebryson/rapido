use anyhow::ensure;
use exonum_crypto::PublicKey;

const DEFAULT_VER_KEY_TYPE: &str = "Ed25519VerificationKey2018";

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

// Parse the keyid from a given DID.
// Ex: given 'did:rapido:CqXbDhD4tLYqmJ9r6w1U76VcEwHp1gzeESsdFJ6H3Mgw#1234'
// it returns '#1234'
fn parse_authentication_key(did: String) -> Result<String, anyhow::Error> {
    ensure!(did.contains("#"), "no keyid specified");

    let parts: Vec<&str> = did.split("#").collect();
    ensure!(parts.len() == 2, "only 1 keyid allowed");

    let keyid = parts.get(1).unwrap();
    ensure!(keyid.len() > 0, "no keyid specified");

    Ok(format!("#{}", keyid))
}

mod tests {
    use super::*;
    use exonum_crypto::gen_keypair;

    #[test]
    fn did_basics() {
        let did: &str = "did:rapido:CqXbDhD4tLYqmJ9r6w1U76VcEwHp1gzeESsdFJ6H3Mgw";
        //let (pk, _sk) = gen_keypair();
        //let did = generate_did(pk);
        //println!("{:}", did);

        assert_eq!(
            "#123",
            parse_authentication_key(
                "did:rapido:CqXbDhD4tLYqmJ9r6w1U76VcEwHp1gzeESsdFJ6H3Mgw#123".into()
            )
            .unwrap()
        );

        // Requires a keyid
        assert!(parse_authentication_key(
            "did:rapido:CqXbDhD4tLYqmJ9r6w1U76VcEwHp1gzeESsdFJ6H3Mgw".into()
        )
        .is_err());

        // Only 1 keyid
        assert!(parse_authentication_key(
            "did:rapido:CqXbDhD4tLYqmJ9r6w1U76VcEwHp1gzeESsdFJ6H3Mgw#123#456".into()
        )
        .is_err());

        // must have some content
        assert!(parse_authentication_key(
            "did:rapido:CqXbDhD4tLYqmJ9r6w1U76VcEwHp1gzeESsdFJ6H3Mgw#".into()
        )
        .is_err());
    }
}
