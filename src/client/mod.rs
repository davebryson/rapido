/// Super simple RPC Client
use url::Url;
use hyper::header;

use::rapido::Transaction;



pub struct Client {
    url: Url,
}

impl Client {
    pub fn new(url: &str) -> Self {
        Self {
            url: Url::parse(url).expect("invalid url"),
        }
    }

    pub fn info(&self) {

    }

    pub fn send_tx(&self) {

    }

    pub fn query(&self) {

    }

    fn perform(&self) {
        let h = self.url.host_str().unwrap();
        let p = self.url.port().unwrap();
        let endpoint = format!("http://{}:{}/",h,p);

        let mut headers = hyper::header::Headers::new();
        headers.set(header::Connection::close());
        headers.set(header::ContentType::json());
        headers.set(header::UserAgent("tendermint.rs RPC client".to_owned()));

        let http_client = hyper::Client::new();

        let mut res = http_client
            .request(hyper::Post, &endpoint)
            .headers(headers)
            .body(&request_body[..])
            .send()
            .map_err(Error::server_error)?;

        let mut response_body = Vec::new();
        res.read_to_end(&mut response_body)
            .map_err(Error::server_error)?;

        println!(response_body);
    }
}
