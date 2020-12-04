//!
//! Create and run the application
use counter::CounterHandler;
use rapido_core::AppBuilder;

fn main() {
    // Configure the application by adding our example.
    // then call `run` to start the ABCI server that
    // connects to Tendermint.
    // Note: uses an in-memory store for testing
    AppBuilder::new().with_app(CounterHandler {}).run();
}
