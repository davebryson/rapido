use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{BinaryKey, ObjectHash};
use failure::ensure;
use std::{convert::TryFrom, fmt, str::FromStr};

/// Size of the Account Address
pub const ACCT_ADDRESS_LENGTH: usize = 32;

// Could make this generic for the app
pub type AccountAddressResult = Result<AccountAddress, failure::Error>;

/// AccountAddress is a container for unique account identifiers.  An AccountAddress
/// can be anything you want as long as it fits into a 32 byte array.
#[derive(BorshSerialize, BorshDeserialize, Ord, PartialOrd, Eq, PartialEq, Default, Clone, Copy)]
pub struct AccountAddress([u8; ACCT_ADDRESS_LENGTH]);

impl AccountAddress {
    /// Create a new AccountAddress
    pub const fn new(address: [u8; ACCT_ADDRESS_LENGTH]) -> Self {
        Self(address)
    }

    /// Convert the AccountAddress to a Vec<u8>
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

// Needed by exonumdb to encode keys in storage
impl BinaryKey for AccountAddress {
    fn size(&self) -> usize {
        ACCT_ADDRESS_LENGTH
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer[..self.size()].copy_from_slice(&self.0);
        self.size()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        if buffer.len() != ACCT_ADDRESS_LENGTH {
            panic!("binary key can read the account address: wrong length")
        }
        let mut addr = [0u8; ACCT_ADDRESS_LENGTH];
        addr.copy_from_slice(buffer);
        AccountAddress(addr)
    }
}

// required for BinaryKey
impl ObjectHash for AccountAddress {
    fn object_hash(&self) -> Hash {
        Hash::new(self.0)
    }
}

impl AsRef<[u8]> for AccountAddress {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for AccountAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:#x}", self)
    }
}

impl fmt::Debug for AccountAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self)
    }
}

impl fmt::LowerHex for AccountAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

impl TryFrom<Hash> for AccountAddress {
    type Error = failure::Error;

    /// Tries to convert the Hash into Address.
    fn try_from(bytes: Hash) -> AccountAddressResult {
        AccountAddress::try_from(&bytes[..])
    }
}

impl TryFrom<&[u8]> for AccountAddress {
    type Error = failure::Error;

    fn try_from(bytes: &[u8]) -> AccountAddressResult {
        ensure!(
            bytes.len() == ACCT_ADDRESS_LENGTH,
            "The Address {:?} is of invalid length",
            bytes
        );
        let mut addr = [0u8; ACCT_ADDRESS_LENGTH];
        addr.copy_from_slice(bytes);
        Ok(AccountAddress(addr))
    }
}

impl TryFrom<Vec<u8>> for AccountAddress {
    type Error = failure::Error;

    fn try_from(bytes: Vec<u8>) -> AccountAddressResult {
        AccountAddress::try_from(&bytes[..])
    }
}

impl From<AccountAddress> for Vec<u8> {
    fn from(addr: AccountAddress) -> Vec<u8> {
        addr.0.to_vec()
    }
}

impl From<&AccountAddress> for Vec<u8> {
    fn from(addr: &AccountAddress) -> Vec<u8> {
        addr.0.to_vec()
    }
}

impl From<&AccountAddress> for String {
    fn from(addr: &AccountAddress) -> String {
        hex::encode(addr.as_ref())
    }
}

impl TryFrom<String> for AccountAddress {
    type Error = failure::Error;

    fn try_from(s: String) -> AccountAddressResult {
        assert!(!s.is_empty());
        let bytes_out = hex::decode(s)?;
        AccountAddress::try_from(bytes_out.as_slice())
    }
}

impl FromStr for AccountAddress {
    type Err = failure::Error;

    fn from_str(s: &str) -> AccountAddressResult {
        assert!(!s.is_empty());
        let bytes_out = hex::decode(s)?;
        AccountAddress::try_from(bytes_out.as_slice())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use exonum_merkledb::{Database, ObjectAccess, ProofMapIndex, RefMut, TemporaryDB};
    use std::sync::Arc;

    // Sample store to test AccountAddress as a store key
    struct SampleStore<T: ObjectAccess>(T);
    impl<T: ObjectAccess> SampleStore<T> {
        pub fn new(object_access: T) -> Self {
            Self(object_access)
        }

        pub fn state(&self) -> RefMut<ProofMapIndex<T, AccountAddress, u8>> {
            self.0.get_object("sample")
        }
    }

    #[test]
    fn test_address_vector() {
        let expected = vec![1u8; 32];
        let addy = AccountAddress::new([1u8; 32]);

        let addy_vec = addy.to_vec();
        assert_eq!(expected, addy_vec);
        assert_eq!(32, addy_vec.len());

        // Into / From
        let t: Vec<u8> = addy.into();
        assert_eq!(expected, t);
        assert_eq!(addy, AccountAddress::try_from(expected).unwrap());
    }

    #[test]
    fn test_address_string() {
        let addy = AccountAddress::new([1u8; 32]);

        let hexed: String = String::from(&addy);
        let b: AccountAddress = AccountAddress::from_str(&hexed).unwrap();
        assert_eq!(b, addy);
    }

    #[test]
    fn test_address_as_store_key() {
        let db = Arc::new(TemporaryDB::new());

        {
            let fork = db.fork();
            let store = SampleStore::new(&fork);
            store.state().put(&AccountAddress::new([1u8; 32]), 1);
            store.state().put(&AccountAddress::new([2u8; 32]), 2);
            assert!(db.merge(fork.into_patch()).is_ok());
        }

        {
            let snap = db.snapshot();
            let store = SampleStore::new(&snap);
            assert_eq!(
                store.state().get(&AccountAddress::new([1u8; 32])).unwrap(),
                1
            );
            assert_eq!(
                store.state().get(&AccountAddress::new([2u8; 32])).unwrap(),
                2
            );
        }
    }

    #[test]
    fn test_acc_address_codec() {
        let a = AccountAddress::new([1u8; 32]);
        let a_bits = a.try_to_vec().unwrap();
        let b = AccountAddress::try_from_slice(&a_bits[..]).unwrap();
        assert_eq!(a, b);
    }

}
