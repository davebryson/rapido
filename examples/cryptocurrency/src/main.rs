use exonum_merkledb::TemporaryDB;
use rapido::AppBuilder;
use std::sync::Arc;

use cryptocurrency::CryptocurrencyService;

fn main() {
    let db = Arc::new(TemporaryDB::new());
    let node = AppBuilder::new(db)
        .add_service(Box::new(CryptocurrencyService {}))
        .finish();
    abci::run_local(node);
}
