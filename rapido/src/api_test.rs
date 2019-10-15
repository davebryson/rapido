use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::{gen_keypair, Hash};
use exonum_merkledb::{Database, Fork, Snapshot, TemporaryDB};
use std::sync::Arc;

use super::{
    sign_transaction, verify_tx_signature, AccountAddress, QueryResult, Service, SignedTransaction,
    Transaction, TxResult,
};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
struct HelloMsgOne(u8);
impl Transaction for HelloMsgOne {
    fn execute(&self, _sender: AccountAddress, _fork: &Fork) -> TxResult {
        if self.0 > 1 {
            return TxResult::error(1, "nope 1");
        }
        TxResult::ok()
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default)]
struct HelloMsgTwo(u8);
impl Transaction for HelloMsgTwo {
    fn execute(&self, _sender: AccountAddress, _fork: &Fork) -> TxResult {
        if self.0 > 2 {
            return TxResult::error(1, "nope 2");
        }
        TxResult::ok()
    }
}

struct MyService;
impl Service for MyService {
    fn route(&self) -> String {
        String::from("myservice")
    }

    fn decode_tx(
        &self,
        txid: u8,
        payload: Vec<u8>,
    ) -> Result<Box<dyn Transaction>, std::io::Error> {
        match txid {
            0 => {
                let m = HelloMsgOne::try_from_slice(&payload[..])?;
                Ok(Box::new(m))
            }
            1 => {
                let m = HelloMsgTwo::try_from_slice(&payload[..])?;
                Ok(Box::new(m))
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "msg not found",
            )),
        }
    }

    fn query(&self, _path: String, _key: Vec<u8>, _snapshot: &Box<dyn Snapshot>) -> QueryResult {
        QueryResult::ok(vec![])
    }

    fn root_hash(&self, _fork: &Fork) -> Hash {
        Hash::zero()
    }
}

#[test]
fn test_basic_service_flow() {
    let db = Arc::new(TemporaryDB::new());

    let msg1 = HelloMsgOne(1);
    let msg2 = HelloMsgTwo(2);
    let enc_msg1 = msg1.try_to_vec().unwrap();
    let enc_msg2 = msg2.try_to_vec().unwrap();

    let fork = db.fork();
    let service = MyService {};

    let tx1 = service.decode_tx(0, enc_msg1).unwrap();
    let result1 = tx1.execute(AccountAddress::new([1u8; 32]), &fork);
    assert_eq!(0u32, result1.code);

    {
        let tx2 = service.decode_tx(1, enc_msg2.clone()).unwrap();
        let result2 = tx2.execute(AccountAddress::new([2u8; 32]), &fork);
        assert_eq!(0u32, result2.code);
    }

    {
        let tx2 = service.decode_tx(0, enc_msg2.clone()).unwrap();
        // Wrong msg id ============ ^
        let result2 = tx2.execute(AccountAddress::new([2u8; 32]), &fork);
        assert_eq!(1u32, result2.code);
    }
}

#[test]
fn test_with_signed_tx() {
    let (pk, sk) = gen_keypair();
    let msg = HelloMsgOne(1u8);

    let mut signed = SignedTransaction::new(AccountAddress::new([1u8; 32]), "hello", 0, msg);
    sign_transaction(&mut signed, &sk);
    assert!(verify_tx_signature(&signed, &pk));

    assert_eq!(AccountAddress::new([1u8; 32]), signed.sender);
    assert_eq!(String::from("hello"), signed.route);
    assert_eq!(0, signed.txid);

    let bits = signed.try_to_vec().unwrap();
    let back = SignedTransaction::try_from_slice(&bits[..]).unwrap();
    let msgback = HelloMsgOne::try_from_slice(&back.payload[..]).unwrap();
    assert_eq!(1u8, msgback.0);
}
