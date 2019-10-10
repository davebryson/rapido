use super::AccountId;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{
    hash, sign, verify, CryptoHash, Hash, PublicKey, SecretKey, Signature, SIGNATURE_LENGTH,
};
use failure::ensure;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct Transaction {
    pub route: String,
    pub msgtype: u8,
    pub msg: Vec<u8>,
    pub signature: Vec<u8>,
    pub signer: AccountId,
}

impl Transaction {
    pub fn new<R, M>(route: R, msgtype: u8, msg: M) -> Self
    where
        R: Into<String>,
        M: BorshSerialize + BorshDeserialize,
    {
        let raw = msg.try_to_vec().unwrap();
        Self {
            route: route.into(),
            msgtype,
            msg: raw,
            signature: Default::default(),
            signer: AccountId::default(),
        }
    }

    pub fn sign(
        &mut self,
        account: &AccountId,
        private_key: &SecretKey,
    ) -> Result<(), failure::Error> {
        ensure!(account.len() > 0, "AccountId is empty");
        self.signer = account.clone();
        // hash contents
        let hashed_content = self.hash();
        // sign
        self.signature = sign(&hashed_content[..], private_key).as_ref().into();
        ensure!(
            SIGNATURE_LENGTH == self.signature.len(),
            "Signature is not the required length"
        );
        Ok(())
    }
}

impl CryptoHash for Transaction {
    fn hash(&self) -> Hash {
        let contents: Vec<u8> = vec![
            self.signer.clone(),
            self.route.as_bytes().to_vec(),
            vec![self.msgtype],
            self.msg.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();
        hash(&contents[..])
    }
}

pub fn verify_tx_signature(tx: &Transaction, public_key: &PublicKey) -> bool {
    let hashed = tx.hash();
    let signature = Signature::from_slice(&tx.signature[..]).unwrap();
    verify(&signature, &hashed[..], public_key)
}
