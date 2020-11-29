use abci::*;
use protobuf::RepeatedField;

// ResponseDeliveryTx
// has many Events
// Event has many Pairs
// example Event:
//         type=transfer
//         name="sender"
//         value="dave"

fn create_event(name: &str, key: &str, value: &str) -> Event {
    let mut p = Pair::new();
    p.set_key(key.as_bytes().to_vec());
    p.set_value(value.as_bytes().to_vec());

    let mut rf = RepeatedField::new();
    rf.push(p);

    let mut e = Event::new();
    e.set_field_type(name.into());
    e.set_attributes(rf);
    e
}

pub struct EventManager<'a> {
    pub appname: &'a str,
    pub events: RepeatedField<Event>,
}

impl<'a> EventManager<'a> {
    pub fn new(appname: &'a str) -> Self {
        Self {
            appname,
            events: RepeatedField::new(),
        }
    }

    /// Example:
    /// let pairs = &[("name", "bob"), ("employer", "Acme")];
    /// eventmanager.emit_event(pairs);
    pub fn dispatch_event(&mut self, event_type: &str, pairs: &[(&str, &str)]) {
        let mut rf = RepeatedField::<Pair>::new();
        for (k, v) in pairs {
            let mut p = Pair::new();
            p.set_key(k.as_bytes().to_vec());
            p.set_value(v.as_bytes().to_vec());
            rf.push(p);
        }

        // Create a type with the appname: 'hello.transfer'
        let full_event_type = format!("{}.{}", self.appname, event_type);
        let mut e = Event::new();
        e.set_field_type(full_event_type.into());
        e.set_attributes(rf);
        self.events.push(e);
    }
}

#[test]
fn test_events() {
    let mut em = EventManager::new("hello");
    let pairs = &[("name", "bob"), ("employer", "Acme")];
    em.dispatch_event("transfer", pairs);

    println!("{:#?}", em.events)
}
