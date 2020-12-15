//!
//! Basic account support with an authenticator. Primarly used for development/testing.
//! Uses a 'Trust Anchor' approach to bootstrapping users: Genesis accounts can create other accounts.
//!
use anyhow::{bail, ensure};
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{hash, PublicKey, PUBLIC_KEY_LENGTH};
use rapido_core::{
    verify_tx_signature, AccountId, AppModule, Authenticator, Context, SignedTransaction, Store,
    StoreView,
};

#[macro_use]
extern crate rapido_core;

const ACCOUNT_APP_NAME: &str = "rapido.account";
const ACCOUNT_STORE_NAME: &str = "rapido.account.store";

pub type PublicKeyBytes = [u8; PUBLIC_KEY_LENGTH];

// Format of the account id: base58(hash(pubkey))
fn generate_account_id(pk: &PublicKey) -> Vec<u8> {
    let hash = hash(&pk.as_bytes());
    bs58::encode(&hash.as_bytes()).into_vec()
}

/// Account Model
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq, Clone)]
pub struct Account {
    pub id: AccountId,
    pub nonce: u64,
    pub pubkey: PublicKeyBytes,
    // flag: can this entity create accounts
    trustanchor: bool,
}

impl Account {
    /// Create a new account given a public key
    pub fn create(pk: &PublicKey, is_ta: bool) -> Self {
        Self {
            id: generate_account_id(pk),
            nonce: 0u64,
            pubkey: pk.as_bytes(),
            trustanchor: is_ta,
        }
    }

    pub fn id(&self) -> Vec<u8> {
        self.id.clone()
    }

    /// Return the base58 account id
    pub fn id_to_str(&self) -> anyhow::Result<String, anyhow::Error> {
        let i = String::from_utf8(self.id.clone());
        ensure!(i.is_ok(), "problem decoding account id to string");
        Ok(i.unwrap())
    }

    /// Is the account a trust anchor?
    pub fn is_trust_anchor(&self) -> bool {
        self.trustanchor
    }

    pub fn update_pubkey(&self, pk: PublicKeyBytes) -> Self {
        Self {
            id: self.id.clone(),
            nonce: self.nonce,
            pubkey: pk,
            trustanchor: self.trustanchor,
        }
    }

    /// Increment the nonce for the account
    pub fn increment_nonce(&self) -> Self {
        Self {
            id: self.id.clone(),
            nonce: self.nonce + 1,
            pubkey: self.pubkey,
            trustanchor: self.trustanchor,
        }
    }
}

impl_store_values!(Account);

/// Account Store
pub(crate) struct AccountStore;
impl Store for AccountStore {
    type Key = AccountId;
    type Value = Account;

    fn name(&self) -> String {
        ACCOUNT_STORE_NAME.into()
    }
}

impl AccountStore {
    pub fn new() -> Self {
        AccountStore {}
    }
}

/// Message used in Transactions
#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq, Clone)]
pub enum Msgs {
    Create(PublicKeyBytes),
    ChangePubKey(PublicKeyBytes),
}

pub struct AccountModule {
    // PublicKeys of genesis accounts
    genesis: Vec<[u8; 32]>,
}

impl AccountModule {
    pub fn new(genesis: Vec<[u8; 32]>) -> Self {
        Self { genesis }
    }
}

impl AppModule for AccountModule {
    fn name(&self) -> String {
        ACCOUNT_APP_NAME.into()
    }

    // Load genesis accounts.  These entries become the trust anchors
    fn initialize(&self, view: &mut StoreView) -> Result<(), anyhow::Error> {
        let store = AccountStore::new();
        for pk in &self.genesis {
            let pubkey = PublicKey::from_slice(&pk[..]).expect("genesis: decode public key");
            let account = Account::create(&pubkey, true); // <= make them a trust anchor
            store.put(account.id(), account, view)
        }
        Ok(())
    }

    fn handle_tx(&self, ctx: &Context, view: &mut StoreView) -> Result<(), anyhow::Error> {
        let msg: Msgs = ctx.decode_msg()?;
        match msg {
            // Create an account.  The origin of this call, must be a trust anchor
            Msgs::Create(pubkey) => {
                let store = AccountStore::new();
                // Ensure the caller's account exists and they are a trust anchor
                let caller_acct = store.get(ctx.sender(), &view);
                ensure!(caller_acct.is_some(), "user not found");
                let acct = caller_acct.unwrap();

                ensure!(
                    acct.is_trust_anchor(),
                    "only a trust anchor can create an account"
                );

                let pk = PublicKey::from_slice(&pubkey[..]);
                ensure!(pk.is_some(), "problem decoding the public key");

                // Create the new account
                let new_account = Account::create(&pk.unwrap(), false);
                store.put(new_account.id(), new_account, view);
                Ok(())
            }

            // Change an existing publickey.  The origin of this call is the owner
            // of the publickey
            Msgs::ChangePubKey(pubkey) => {
                let store = AccountStore::new();
                let caller_acct = store.get(ctx.sender(), &view);
                ensure!(caller_acct.is_some(), "user not found");
                let acct = caller_acct.unwrap();

                let updated = acct.update_pubkey(pubkey);
                store.put(updated.id(), updated, view);
                Ok(())
            }
        }
    }

    fn handle_query(
        &self,
        path: &str,
        key: Vec<u8>,
        view: &StoreView,
    ) -> Result<Vec<u8>, anyhow::Error> {
        ensure!(key.len() > 0, "bad account key");

        // return a serialized account for the given id.
        match path {
            "/" => {
                let account = key;
                let store = AccountStore::new();
                let req_acct = store.get(account, &view);
                ensure!(req_acct.is_some(), "account not found");

                let acct: Account = req_acct.unwrap();
                let bits = acct.try_to_vec()?;
                Ok(bits)
            }
            _ => bail!("{:} not found", path),
        }
    }
}

// Authenticator
pub struct AccountAuthenticator;
impl Authenticator for AccountAuthenticator {
    fn validate(
        &self,
        tx: &SignedTransaction,
        view: &StoreView,
    ) -> anyhow::Result<(), anyhow::Error> {
        let caller = tx.sender();
        let txnonce = tx.nonce();
        let store = AccountStore::new();

        let caller_acct = store.get(caller, &view);
        ensure!(caller_acct.is_some(), "user not found");
        let acct = caller_acct.unwrap();

        let caller_pubkey = PublicKey::from_slice(&acct.pubkey[..]);
        ensure!(
            caller_pubkey.is_some(),
            "problem decoding the user's public key"
        );

        // Validate signature
        ensure!(
            verify_tx_signature(&tx, &caller_pubkey.unwrap()),
            "bad signature"
        );

        // Check nonce
        ensure!(acct.nonce == txnonce, "nonce don't match");

        Ok(())
    }

    fn increment_nonce(
        &self,
        tx: &SignedTransaction,
        view: &mut StoreView,
    ) -> anyhow::Result<(), anyhow::Error> {
        let caller = tx.sender();
        let store = AccountStore::new();
        let caller_acct = store.get(caller.clone(), &view);
        ensure!(caller_acct.is_some(), "user not found");

        let acct = caller_acct.unwrap();
        let unonce = acct.increment_nonce();
        store.put(caller.clone(), unonce, view);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exonum_crypto::{gen_keypair, SecretKey};
    use rapido_core::{testing_keypair, AppBuilder, TestKit};

    fn create_account(name: &str) -> (Vec<u8>, PublicKeyBytes, SecretKey) {
        let (pk, sk) = testing_keypair(name);
        let acct = Account::create(&pk, true);
        (acct.id(), acct.pubkey, sk)
    }

    fn get_genesis_accounts() -> Vec<[u8; 32]> {
        vec![
            create_account("bob").1,
            create_account("alice").1,
            create_account("tom").1,
        ]
    }

    fn gen_tx(user: Vec<u8>, secret_key: &SecretKey, nonce: u64) -> SignedTransaction {
        let mut tx = SignedTransaction::create(
            user,
            ACCOUNT_APP_NAME,
            Msgs::Create([1u8; 32]), // fake data
            nonce,
        );
        tx.sign(&secret_key);
        tx
    }

    #[test]
    fn test_account_authenticator() {
        // Check signature verification and nonce rules are enforced
        let app = AppBuilder::new()
            .set_authenticator(AccountAuthenticator {})
            .with_app(AccountModule::new(get_genesis_accounts()));

        let mut tester = TestKit::create(app);
        tester.start();

        let (bob, _bpk, bsk) = create_account("bob");

        // Check signatures and correct nonce
        let txs = &[
            &gen_tx(bob.clone(), &bsk, 0u64),
            &gen_tx(bob.clone(), &bsk, 1u64),
            &gen_tx(bob.clone(), &bsk, 2u64),
            &gen_tx(bob.clone(), &bsk, 3u64),
        ];

        assert!(tester.check_tx(txs).is_ok());

        // Wrong nonce
        assert!(tester
            .check_tx(&[&gen_tx(bob.clone(), &bsk, 5u64)])
            .is_err());

        // Bad signature: bob's ID but signed with wrong key
        let (_rpk, rsk) = gen_keypair();
        assert!(tester
            .check_tx(&[&gen_tx(bob.clone(), &rsk, 0u64)])
            .is_err());
    }

    #[test]
    fn test_ta_account_create() {
        // Bob will create an account for Carol
        // Carol will try to create an account for Andy...but it'll fail
        let app = AppBuilder::new()
            .set_authenticator(AccountAuthenticator {})
            .with_app(AccountModule::new(get_genesis_accounts()));

        let mut tester = TestKit::create(app);
        tester.start();

        let (bob, _bpk, bsk) = create_account("bob");
        let (carol, cpk, csk) = create_account("carol");
        let (_andy, apk, _) = create_account("andy");

        let mut tx =
            SignedTransaction::create(bob.clone(), ACCOUNT_APP_NAME, Msgs::Create(cpk), 0u64);
        tx.sign(&bsk);

        assert!(tester.check_tx(&[&tx]).is_ok());
        assert!(tester.commit_tx(&[&tx]).is_ok());

        assert!(tester.query("rapido.account", carol.clone()).is_ok());

        let mut tx1 =
            SignedTransaction::create(carol.clone(), ACCOUNT_APP_NAME, Msgs::Create(apk), 0u64);
        tx1.sign(&csk);

        // Check passes...but
        assert!(tester.check_tx(&[&tx1]).is_ok());
        // deliver fails...carol is not a TA
        assert!(tester.commit_tx(&[&tx1]).is_err());
    }

    #[test]
    fn test_account_chng_pubkey() {
        // Bob will change is pubkey.  Make sure he can authenticate with it

        assert!(true)
    }
}
