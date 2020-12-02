//! Counter application that's simple and demonstrates Rapido functionality.
//! User's can maintain a counter in a state store.
//! A user account (tx.sender) is just a 'name'.  So:
//! name => Counter{}
//! User's can increase and decrease their Counters and check the current count.
//! For demo purposes, Txs don't need to be signed.

use borsh::{BorshDeserialize, BorshSerialize};
use rapido_core::{AccountId, AppModule, Context, Store, StoreView};

#[macro_use]
extern crate rapido_core;

#[macro_use]
extern crate log;

pub const APP_NAME: &'static str = "counter.app";

/// Model for the counter.  This is stored in the Merkle Tree
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Default)]
pub struct Counter {
    count: u16,
}

impl Counter {
    pub fn add(&self, value: u16) -> Self {
        Self {
            count: self.count + value,
        }
    }

    pub fn subtract(&self, value: u16) -> Self {
        Self {
            count: self.count - value,
        }
    }

    pub fn to_hex(&self) -> String {
        format!("{:x}", self.count)
    }
}

// Call to make it storable in the Tree
impl_store_values!(Counter);

/// Store for counters
pub struct CounterStore;
impl Store for CounterStore {
    // The key for the store.  Counter name
    type Key = AccountId;
    // The actual value stored
    type Value = Counter;

    // TODO: Change this to &'static str
    fn name(&self) -> String {
        "counter.store".into()
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum Msgs {
    Create(String),
    Add(u16),
    Subtract(u16),
}

pub struct CounterHandler;
impl AppModule for CounterHandler {
    fn name(&self) -> &'static str {
        APP_NAME
    }

    fn initialize(&self, _view: &mut StoreView) -> Result<(), anyhow::Error> {
        debug!("counter run init");
        Ok(())
    }

    fn handle_tx(&self, ctx: &Context, view: &mut StoreView) -> Result<(), anyhow::Error> {
        let msg: Msgs = ctx.decode_msg()?;
        let store = CounterStore {};
        debug!("counter: handle tx!");

        match msg {
            Msgs::Create(name) => {
                let n = name.as_bytes().to_vec();
                ensure!(store.get(n.clone(), view).is_none(), "User already exists");
                store.put(n, Counter::default(), view);
                Ok(())
            }
            Msgs::Add(val) => {
                if let Some(cnt) = store.get(ctx.sender.clone(), view) {
                    store.put(ctx.sender.clone(), cnt.add(val), view);
                    return Ok(());
                }
                bail!("user not found")
            }
            Msgs::Subtract(val) => {
                if let Some(cnt) = store.get(ctx.sender.clone(), view) {
                    if val > cnt.count {
                        bail!("can't have negative results from a subtract")
                    }
                    store.put(ctx.sender.clone(), cnt.subtract(val), view);
                    return Ok(());
                }
                bail!("user not found")
            }
        }
    }

    fn handle_query(
        &self,
        path: &str,
        key: Vec<u8>,
        view: &StoreView,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let account = key;
        //ensure!(account.is_ok(), "Error parsing the query key!");
        let store = CounterStore {};
        //let user = account.unwrap();
        match path {
            "/" => match store.get(account.clone(), view) {
                Some(c) => Ok(c.try_to_vec().unwrap()),
                None => bail!("not count found for the given user"),
            },
            _ => bail!("nothing else to see here..."),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
