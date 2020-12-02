use counter::CounterHandler;
use rapido_core::AppBuilder;

fn main() {
    AppBuilder::new().with_app(CounterHandler {}).run();
}
