use std::path::PathBuf;

use exonum_merkledb::{DbOptions, RocksDB};

const RAPIDO_HOME: &str = ".rapido";
const RAPIDO_STATE_DIR: &str = "state";

pub fn dbdir() -> PathBuf {
    let mut dir = dirs::home_dir().expect("find home dir");
    dir.push(RAPIDO_HOME);
    dir.push(RAPIDO_STATE_DIR);
    dir
}

pub fn purge_state_db() -> std::io::Result<()> {
    std::fs::remove_dir_all(dbdir())
}

#[test]
fn test_config() {
    //let mut d = dirs::home_dir().unwrap();
    //d.push(".rapido");
    //d.push("state");
    //println!("{:?}", d);
    //let _db = RocksDB::open(dbdir(), &DbOptions::default());
    //purge_db();
}
