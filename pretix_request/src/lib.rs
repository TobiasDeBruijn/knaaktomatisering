use std::ops::Deref;

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;

pub mod data_exporter;
pub mod events;
pub mod oauth;
pub mod organizer;

pub struct PretixClient {
    client: Client,
    pretix_url: String,
}

impl PretixClient {
    pub fn new<S: AsRef<str>>(access_token: S, pretix_url: String) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", access_token.as_ref()))
                .expect("Creating authorization header value"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .user_agent("Sticky Knaaktomatisering")
            .build()
            .expect("Creating Pretix request client");

        Self { client, pretix_url }
    }

    pub fn url<S: AsRef<str>>(&self, path: S) -> String {
        format!("{}{}", self.pretix_url, path.as_ref())
    }

    /// List all values from an endpoint that is paginated, e.g. `/api/v1/organizers`
    pub async fn list_paginated<S: AsRef<str>, T: DeserializeOwned>(
        &self,
        url: S,
    ) -> reqwest::Result<Vec<T>> {
        let response: PretixListResponse<T> = self
            .get(url.as_ref())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut data = response.results;

        let mut next = response.next;
        while let Some(next_url) = next {
            let mut results: PretixListResponse<T> = self
                .get(next_url)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            next = results.next;
            data.append(&mut results.results)
        }

        Ok(data)
    }
}

impl Deref for PretixClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

#[derive(Debug, Deserialize)]
struct PretixListResponse<T> {
    next: Option<String>,
    results: Vec<T>,
}
