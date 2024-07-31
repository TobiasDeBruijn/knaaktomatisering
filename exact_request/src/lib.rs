use log::trace;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::ops::Deref;
use thiserror::Error;

pub mod api;

#[derive(Debug, Error)]
pub enum ExactError {
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("{0}")]
    NoAccountingDivision(#[from] NoDivisionError),
}

pub struct ExactClient {
    client: Client,
    accounting_division: Option<i32>,
}

#[derive(Debug, Error)]
#[error("No accounting division was set")]
pub struct NoDivisionError;

impl ExactClient {
    pub fn new<S: AsRef<str>>(access_token: S) -> Self {
        let mut hm = HeaderMap::new();
        hm.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", access_token.as_ref())).unwrap(),
        );
        hm.insert("Accept", HeaderValue::from_static("application/json"));

        let client = Client::builder()
            .user_agent(format!(
                "{} v{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ))
            .default_headers(hm)
            .build()
            .expect("Creating Exact client");

        Self {
            client,
            accounting_division: None,
        }
    }

    /// Format a URL for Exact Online.
    /// If the Exact documentation specifies `/api/v1/current/Me`, pass that to this function.
    /// You should only use this for endpoints that do not require the accounting division. The accounting
    /// division is the number after `/api/v1/`. For example, in `/api/v1/55861/salesentry/SalesEntries`
    /// the accounting division is `55861`. For endpoints with an accounting division, use [Self::divisioned_url].
    pub fn url<S: AsRef<str>>(s: S) -> String {
        format!("https://start.exactonline.nl{}", s.as_ref())
    }

    /// Format a URL within the accounting division namespace.
    /// If the Exact documentation specifies e.g. `/api/v1/55861/salesentry/SalesEntries` as the URL,
    /// you should pass `/salesentry/SalesEntries` to this function.
    ///
    /// # Errors
    ///
    /// If no accounting division is set. To set the accounting division, use [Self::set_division]
    pub fn divisioned_url<S: AsRef<str>>(&self, s: S) -> Result<String, NoDivisionError> {
        let div = self.accounting_division.ok_or(NoDivisionError)?;
        let url = Self::url(format!("/api/v1/{div}{}", s.as_ref()));

        trace!("URL: {url}");

        Ok(url)
    }

    /// Set the accounting division ID. This ID can be obtained with [api::me::accounting_division].
    pub fn set_division(&mut self, accounting_division: i32) {
        self.accounting_division = Some(accounting_division);
    }
}

impl Deref for ExactClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

#[derive(Deserialize)]
pub struct ExactPayload<T> {
    d: ExactData<T>,
}

#[derive(Deserialize)]
pub struct ExactData<T> {
    results: Vec<ExactResult<T>>,
}

#[derive(Deserialize)]
pub struct ExactResult<T> {
    #[serde(flatten)]
    value: T,
}

impl<T: DeserializeOwned> ExactPayload<T> {
    pub fn value(self) -> T {
        self.d.results.into_iter().nth(0).unwrap().value
    }

    pub fn values(self) -> Vec<T> {
        self.d
            .results
            .into_iter()
            .map(|r| r.value)
            .collect::<Vec<_>>()
    }
}
