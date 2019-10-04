use std::sync::Arc;

use super::StateStore;
use exonum_merkledb::{Database, TemporaryDB};

#[test]
fn test_store() {
    let db = Arc::new(TemporaryDB::new());
    let mut batch = StateStore::new(db.clone());

    let acct1 = AccountAddress::new([1u8; 32]);
    let acct2 = AccountAddress::new([2u8; 32]);

    // Init
    {
        assert_eq!(0, batch.size());

        batch.put(
            acct1.clone(),
            AccountData {
                blob: vec![1, 1, 1],
            },
        );

        batch.put(
            acct2.clone(),
            AccountData {
                blob: vec![2, 2, 2],
            },
        );
    }

    // Read it 1
    {
        assert_eq!(2, batch.size());

        let r1 = batch.get(acct2.clone());
        assert!(r1.is_some());
        assert_eq!(vec![2, 2, 2], r1.unwrap().blob);

        let r2 = batch.get(acct1.clone());
        assert!(r2.is_some());
        assert_eq!(vec![1, 1, 1], r2.unwrap().blob);

        let nope = batch.get(AccountAddress::new([99u8; 32]));
        assert!(nope.is_none());
    }

    // Remove & Commit
    {
        let fork = db.fork();
        batch.remove(acct1.clone());
        batch.commit(&fork);
        let r = db.merge(fork.into_patch());
        assert!(r.is_ok());
    }

    {
        // Cache is flushed after commit!
        assert_eq!(0, batch.size());
        let r2 = batch.get(acct1.clone());
        assert!(r2.is_none());

        let r1 = batch.get(acct2.clone());
        assert!(r1.is_some());
        assert_eq!(vec![2, 2, 2], r1.unwrap().blob);
    }

    // Update & commit
    {
        let a = acct2.clone();
        assert_eq!(1, batch.size());
        let r1 = batch.get(a);
        assert!(r1.is_some());
        let mut data = r1.unwrap();
        data.blob = vec![3, 3, 3];
        batch.put(a, data);

        let r2 = batch.get(a.clone());
        assert!(r2.is_some());
        assert_eq!(vec![3, 3, 3], r2.unwrap().blob);

        let fork = db.fork();
        batch.commit(&fork);
        let r = db.merge(fork.into_patch());
        assert!(r.is_ok());
    }

    {
        assert_eq!(0, batch.size());
        let r2 = batch.get(acct2.clone());
        assert!(r2.is_some());
        assert_eq!(vec![3, 3, 3], r2.unwrap().blob);
    }
}
