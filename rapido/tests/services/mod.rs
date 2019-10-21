pub mod accounts;
pub mod proofodna;

use accounts::GenesisAccounts;
use exonum_crypto::{gen_keypair, hash, PublicKey, SecretKey};

/// Simple Wallet used for testing
pub struct TestWallet {
    wallets: Vec<(Vec<u8>, u8, SecretKey, PublicKey)>,
    num_wallets: usize,
}

impl TestWallet {
    pub fn generate(num: usize) -> Self {
        let mut w: Vec<(Vec<u8>, u8, SecretKey, PublicKey)> = Vec::new();
        for _ in 0..=num {
            let (pk, sk) = gen_keypair();
            let address = hash(&pk[..]);
            w.push((address[..].to_vec(), 5, sk, pk))
        }
        Self {
            wallets: w,
            num_wallets: num,
        }
    }

    pub fn get(&self, index: usize) -> Option<&(Vec<u8>, u8, SecretKey, PublicKey)> {
        self.wallets.get(index)
    }

    pub fn get_address(&self, index: usize) -> Vec<u8> {
        self.get(index).unwrap().0.clone()
    }

    pub fn get_secretkey(&self, index: usize) -> SecretKey {
        self.get(index).unwrap().2.clone()
    }

    pub fn get_publickey(&self, index: usize) -> PublicKey {
        self.get(index).unwrap().3.clone()
    }

    pub fn generate_genesis_data(&self) -> Vec<u8> {
        let mut ga = GenesisAccounts::new();
        for (addr, bal, _, pk) in &self.wallets {
            let mut buf = [0u8; 32];
            buf.copy_from_slice(&pk[..]);
            ga.add(addr.clone(), *bal, buf);
        }
        ga.encode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::proofodna::RegisterTx;
    use rapido::{sign_transaction, verify_tx_signature, SignedTransaction};

    #[test]
    fn test_wallet() {
        let wallet = TestWallet::generate(3);
        assert_eq!(32, wallet.get(0).unwrap().0.len());
        assert_eq!(5, wallet.get(2).unwrap().1);

        let bob = wallet.get(1).unwrap();

        let mut signed = SignedTransaction::new(
            bob.0.clone(),
            "hello",
            0,
            RegisterTx::new(vec![], [2u8; 32]),
        );
        sign_transaction(&mut signed, &bob.2);
        verify_tx_signature(&signed, &bob.3);
    }
}
