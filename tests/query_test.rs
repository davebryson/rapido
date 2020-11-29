// Experiment with query routing
use anyhow::bail;
use std::collections::HashMap;

// Rules:
// If 'name/' or 'name' => Some((name, "/"))
//    'name/one => ((name, '/one'))
//    'name/one/two => ((name, '/one/two'))
//    '' => None
fn parse_abci_query_path(req_path: &str) -> Option<(&str, &str)> {
    if req_path.len() == 0 {
        return None;
    }
    if req_path == "/" {
        return None;
    }
    if !req_path.contains("/") {
        return Some((req_path, "/"));
    }

    req_path
        .find("/")
        .filter(|i| i > &0usize)
        .and_then(|index| Some(req_path.split_at(index)))
}

type QueryHandler = fn(key: Vec<u8>) -> bool;

struct QueryRouter {
    appname: String,
    map: HashMap<String, Box<QueryHandler>>,
}

impl QueryRouter {
    fn new(appname: &str) -> Self {
        Self {
            appname: appname.into(),
            map: HashMap::new(),
        }
    }

    fn add(&mut self, route: &str, handler: QueryHandler) {
        if !route.starts_with(&self.appname) {
            panic!("nope");
        }
        self.map.insert(route.into(), Box::new(handler));
    }

    fn get(&self, route: &str) -> Option<&Box<QueryHandler>> {
        self.map.get(route)
    }
}

// Example of possible routes:
// Assume appname = 'hello'
// May have:
// ''  for just root: 'hello/'
// '/name' for: 'hello/name'
// '/again/one' for: 'hello/again/one'
#[test]
fn test_parser() {
    assert_eq!(("hello", "/"), parse_abci_query_path("hello").unwrap());
    assert_eq!(("hello", "/"), parse_abci_query_path("hello/").unwrap());
    assert_eq!(None, parse_abci_query_path(""));
    assert_eq!(None, parse_abci_query_path("/"));
    assert_eq!(
        ("hello", "/one"),
        parse_abci_query_path("hello/one").unwrap()
    );
    assert_eq!(
        ("hello", "/one/two"),
        parse_abci_query_path("hello/one/two").unwrap()
    );
}

fn say_hello(_key: Vec<u8>) -> bool {
    true
}

fn handler_ex(path: String) -> Result<(), anyhow::Error> {
    match path.as_str() {
        "hello" => Ok(()),
        "hello/one" => Ok(()),
        _ => bail!("nope"),
    }
}

#[test]
fn test_router() {
    let mut router = QueryRouter::new("hello");
    router.add("hello", say_hello);
    router.add("hello/one", say_hello);
    router.add("hello/one/two", say_hello);

    assert!(router.get("hello").unwrap()(vec![1]));
    assert!(router.get("hello/one").unwrap()(vec![1]));
}

#[test]
fn test_parse() {
    assert_eq!(0, "".find("").unwrap());
    assert_eq!(0, "/".find("/").unwrap());
    assert_eq!(5, "hello/".find("/").unwrap());
}
