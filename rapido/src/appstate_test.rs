use exonum_merkledb::{Database, TemporaryDB};
use std::sync::Arc;

use crate::appstate::{AppState, AppStateSchema};

#[test]
fn test_app_state() {
    let db = Arc::new(TemporaryDB::new());
    {
        // New uses default
        let snapshot = db.snapshot();
        let schema = AppStateSchema::new(&snapshot);
        let app_state = schema.app_state().get().unwrap_or_default();
        assert_eq!(0i64, app_state.version);
        assert_eq!(Vec::<u8>::new(), app_state.hash);
    }

    {
        let f = db.fork();
        let schema = AppStateSchema::new(&f);
        schema.app_state().set(AppState {
            version: 1i64,
            hash: vec![1, 1],
        });
        let r = db.merge(f.into_patch());
        assert!(r.is_ok());
    }

    {
        let snapshot = db.snapshot();
        let schema = AppStateSchema::new(&snapshot);
        let app_state = schema.app_state().get().unwrap_or_default();
        assert_eq!(1i64, app_state.version);
        assert_eq!(vec![1, 1], app_state.hash);
    }

    {
        let f = db.fork();
        let schema = AppStateSchema::new(&f);
        schema.app_state().set(AppState {
            version: 2i64,
            hash: vec![2, 2],
        });
        let r = db.merge(f.into_patch());
        assert!(r.is_ok());
    }

    {
        let snapshot = db.snapshot();
        let schema = AppStateSchema::new(&snapshot);
        let app_state = schema.app_state().get().unwrap_or_default();
        assert_eq!(2i64, app_state.version);
        assert_eq!(vec![2, 2], app_state.hash);
    }
}
