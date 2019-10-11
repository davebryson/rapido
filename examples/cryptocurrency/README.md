# Cryptocurrency Example

Little example of building a Tendermint application with *Rapido*

You need 3 terminal windows:
(1) for the CLI
(2) another for the app 
(3) final one for Tendermint

1. Make sure you have tendermint installed **Muy importante!**
2. Run `tendermint init` (3)
3. Start the app (2) `cargo run --example cryptocurrency run`

Now you're ready to play in the the CLI (1):
**Note account mappings are hard coded. Stick with the accounts: dave, bob, alice, tom**
1. Create an account: `cargo run --example cryptocurrency create "alice"`
2. Deposit to that account: `cargo run --example cryptocurrency deposit "alice" 10`
3. Create another account: `cargo run --example cryptocurrency create "bob"`
4. Transfer 5 tokens from alice => bob: `cargo run --example cryptocurrency transfer "alice" "bob" 5`