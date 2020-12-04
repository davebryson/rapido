//! Simple 'counter' example
//!
//!
//! A user can maintain their own personal counter in the state store. This example uses the default
//! (testing) authenticator that allows all transactions (don't need to signed).  User's can increase
//! and decrease their Counters and check the current count.
//!
use borsh::{BorshDeserialize, BorshSerialize};
use rapido_core::{AccountId, AppModule, Context, Store, StoreView};

#[macro_use]
extern crate rapido_core;

#[macro_use]
extern crate log;

/// Unique name for the application.  This will be registered in the overall application.
/// Use this value to set the 'app' value in a transaction
pub const APP_NAME: &'static str = "example.counter.app";

/// Implement what you want to store (model).  Each user has a Count in the Merkle Tree.  
/// We simple store the count as a u16.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Default)]
pub struct Counter(pub u16);
impl Counter {
    // Add a value to the current count
    pub fn add(&self, value: u16) -> Self {
        Self(self.0 + value)
    }

    // Substract a value to the current count
    pub fn subtract(&self, value: u16) -> anyhow::Result<Self, anyhow::Error> {
        if value > self.0 {
            bail!("I don't do negative results")
        }
        Ok(Self(self.0 - value))
    }
}

// Make `Counter` something that can be stored.  This macro implements the traits required
// by the underlying storage model
impl_store_values!(Counter);

/// Implement the Store for this application.  The `Store trait` already implements the
/// common store operations such as: `put`, `get`, etc....   
pub struct CounterStore;
impl Store for CounterStore {
    /// Set the Key type used by this store.  
    type Key = AccountId;
    /// Set the Value type used by this store.  
    type Value = Counter;

    /// Return the unique name of this store. This value is used internally to prefix all keys.
    /// Keys are prefixed with the `name()` result and then hashed before they're actually stored.
    fn name(&self) -> String {
        "counter.store".into()
    }
}

/// Implement the messages the drive state changes in your AppModule.  `Msgs` are included in
/// the transaction.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum Msgs {
    Create,
    Add(u16),
    Subtract(u16),
}

/// This the core logic for your application.
pub struct CounterHandler;
impl AppModule for CounterHandler {
    /// The unique name of the application
    fn name(&self) -> String {
        APP_NAME.into()
    }

    /// Implement this to load any unique data to this application.  This is genesis
    /// data and only loaded once, on the first start.
    fn initialize(&self, _view: &mut StoreView) -> Result<(), anyhow::Error> {
        debug!("counter run init");
        Ok(())
    }

    /// Called to handle incoming transaction to this application
    fn handle_tx(&self, ctx: &Context, view: &mut StoreView) -> Result<(), anyhow::Error> {
        // Decode the message the was delivered in the transaction
        let msg: Msgs = ctx.decode_msg()?;
        // Get our store
        let store = CounterStore {};
        debug!("counter: handle tx!");

        // Operate on the message based on type
        match msg {
            // Create a new Counter for the sender, as long as they don't have one already
            Msgs::Create => {
                // Ctx contains the sender
                let user = ctx.sender.clone();

                // Check the store. Errs if user exists
                ensure!(
                    store.get(user.clone(), view).is_none(),
                    "User already exists"
                );

                // Store the new user/counter
                store.put(user, Counter::default(), view);
                Ok(())
            }
            Msgs::Add(val) => {
                // Call Add on the user's Counter
                if let Some(cnt) = store.get(ctx.sender.clone(), view) {
                    store.put(ctx.sender.clone(), cnt.add(val), view);
                    return Ok(());
                }
                bail!("user not found")
            }
            Msgs::Subtract(val) => {
                // Call Subtract on the user's counter
                if let Some(cnt) = store.get(ctx.sender.clone(), view) {
                    let new_count = cnt.subtract(val)?;
                    store.put(ctx.sender.clone(), new_count, view);
                    return Ok(());
                }
                bail!("user not found")
            }
        }
    }

    /// Handle RPC queries to the application. You define the relative paths to the App.
    fn handle_query(
        &self,
        path: &str,
        key: Vec<u8>,
        view: &StoreView,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let account = key;

        // This is the only path for this example.  This would respond to the RPC call:
        // http://127.0.0.1:26657/example.counter.app/
        //
        if path == "/" {
            let store = CounterStore {};
            return match store.get(account.clone(), view) {
                Some(c) => Ok(c.try_to_vec().unwrap()), // Convert the counter to a Vec for transport
                None => bail!("not count found for the given user"),
            };
        }
        bail!("nothing else to see here...")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
