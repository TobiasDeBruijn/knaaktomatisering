use reqwest::Result;
use serde::Deserialize;
use std::fmt::Display;

use crate::PretixClient;

#[derive(Debug, Deserialize)]
pub struct Organizer {
    pub name: String,
    pub slug: OrganizerId,
}

#[derive(Debug, Deserialize)]
pub struct OrganizerId(pub String);

impl Organizer {
    /// List all accessible organizers.
    pub async fn list(client: &PretixClient) -> Result<Vec<Organizer>> {
        client
            .list_paginated(client.url("/api/v1/organizers"))
            .await
    }
}

impl Display for OrganizerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
