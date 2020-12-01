use super::Account;
use exonum_crypto::{hash, KeyPair, Seed};

fn generator(v: &str) -> Account {
    let pair = create_keypair(v);
    Account::create(pair.public_key())
}

pub fn create_keypair(v: &str) -> KeyPair {
    let seed = Seed::new(hash(v.as_bytes()).as_bytes());
    KeyPair::from_seed(&seed)
}

/// Return a list of Accounts to use for testing/development
pub fn generate_dev_accounts() -> Vec<Account> {
    vec![generator("/Dave"), generator("/Bob"), generator("/Alice")]
}
