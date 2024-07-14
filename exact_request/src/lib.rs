use std::ops::Deref;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::Deserialize;

pub mod oauth;
pub mod me;

pub fn url<S: AsRef<str>>(s: S) -> String {
    format!("https://start.exactonline.nl/{}", s.as_ref())
}

pub struct ExactClient(Client);

impl ExactClient {
    pub fn new<S: AsRef<str>>(access_token: S) -> Self {
        let mut hm = HeaderMap::new();
        hm.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", access_token.as_ref())).unwrap());
        hm.insert("Accept", HeaderValue::from_static("application/json"));

        let client = Client::builder()
            .user_agent(format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
            .default_headers(hm)
            .build()
            .expect("Creating Exact client");

        Self(client)
    }
}

impl Deref for ExactClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Deserialize)]
pub struct ExactPayload<T> {
    d: ExactData<T>
}

#[derive(Deserialize)]
pub struct ExactData<T> {
    results: Vec<ExactResult<T>>
}

#[derive(Deserialize)]
pub struct ExactResult<T> {
    #[serde(flatten)]
    value: T
}

impl<T: DeserializeOwned> ExactPayload<T> {
    pub fn value(self) -> T {
        self.d
            .results
            .into_iter()
            .nth(0)
            .unwrap()
            .value
    }
}