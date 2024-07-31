use crate::organizer::OrganizerId;
use crate::PretixClient;
use reqwest::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Display;
use time::OffsetDateTime;

#[derive(Debug, Deserialize)]
pub struct Event {
    /// The name of the event.
    /// Key is the language shortcode, e.g. `en`.
    /// Value is the name in the specified language.
    pub name: HashMap<String, String>,
    pub slug: EventId,
    pub live: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub date_from: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub date_to: Option<OffsetDateTime>,
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize)]

pub struct EventId(String);

impl Event {
    /// List all events organized by the specified organizer.
    pub async fn list(client: &PretixClient, organizer: &OrganizerId) -> Result<Vec<Event>> {
        client
            .list_paginated(client.url(format!("/api/v1/organizers/{organizer}/events")))
            .await
    }
}

impl Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
