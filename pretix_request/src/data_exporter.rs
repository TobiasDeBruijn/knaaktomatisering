use crate::events::EventId;
use crate::organizer::OrganizerId;
use crate::PretixClient;
use log::{debug, error};
use reqwest::{Response, Result, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Debug, Deserialize)]
pub struct DataExporter {
    pub identifier: String,
    pub verbose_name: String,
    pub input_parameters: Vec<DataExporterInput>,
}

#[derive(Debug, Deserialize)]
pub struct DataExporterInput {
    pub name: String,
    pub required: bool,
    pub choices: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct OrderDataExportOrderItem {
    pub fees: Vec<OrderDataExportOrderItemFee>,
    #[serde(with = "time::serde::rfc3339")]
    pub datetime: OffsetDateTime,
    pub total: String,
}

#[derive(Debug, Deserialize)]
pub struct OrderDataExportOrderItemFee {
    pub value: String,
}

#[derive(Debug, Error)]
pub enum ExporterError {
    #[error("{0}")]
    Request(#[from] reqwest::Error),
    #[error("{0}")]
    FormatDescription(#[from] time::error::InvalidFormatDescription),
    #[error("{0}")]
    Format(#[from] time::error::Format),
    #[error("Export failed: {reason}")]
    ExportFail { reason: String },
    #[error("Export failed for unknown reason: HTTP {status}")]
    Other { status: StatusCode },
}

impl DataExporter {
    pub async fn list(
        client: &PretixClient,
        organizer: &OrganizerId,
        event: &EventId,
    ) -> Result<Vec<Self>> {
        client
            .list_paginated(client.url(format!(
                "/api/v1/organizers/{organizer}/events/{event}/exporters"
            )))
            .await
    }

    pub async fn export_order_data(
        client: &PretixClient,
        organizer: &OrganizerId,
        event: &EventId,
    ) -> std::result::Result<Vec<OrderDataExportOrderItem>, ExporterError> {
        // Start the export
        let url = Self::run_exporter::<()>(client, organizer, event, "json", None).await?;

        // Format of the response body
        #[derive(Deserialize)]
        struct ExportResponse {
            event: ExportResponseEvent,
        }

        #[derive(Deserialize)]
        struct ExportResponseEvent {
            orders: Vec<OrderDataExportOrderItem>,
        }

        // Deserialize once it's done
        let payload: ExportResponse = Self::wait_for_export(client, url).await?.json().await?;

        Ok(payload.event.orders)
    }

    pub async fn export_order_data_pdf(
        client: &PretixClient,
        organizer: &OrganizerId,
        event: &EventId,
        from: OffsetDateTime,
        until: OffsetDateTime,
    ) -> std::result::Result<Vec<u8>, ExporterError> {
        let formatter = time::format_description::parse("[year]-[month]-[day]")?;
        let from = from.format(&formatter)?;
        let until = until.format(&formatter)?;

        // Start the export
        let url = Self::run_exporter(
            client,
            organizer,
            event,
            "pdfreport",
            Some(HashMap::from([
                ("date_axis", "last_payment_date"),
                ("date_from", &from),
                ("date_until", &until),
            ])),
        )
        .await?;

        // Wait for export completion
        let export = Self::wait_for_export(client, url).await?;
        Ok(export.bytes().await?.to_vec())
    }

    async fn wait_for_export<S: AsRef<str>>(
        client: &PretixClient,
        url: S,
    ) -> std::result::Result<Response, ExporterError> {
        debug!("Waiting for export {} to be ready", url.as_ref());
        let start = Instant::now();

        loop {
            let response = client.get(url.as_ref()).send().await?;

            match response.status() {
                StatusCode::CONFLICT => {
                    // Request is pending
                    debug!("Waiting on export. ({:.2?})", start.elapsed());
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                StatusCode::OK => {
                    // Request is done
                    debug!("Export complete. (took {:.2?})", start.elapsed());
                    return Ok(response);
                }
                StatusCode::GONE => {
                    #[derive(Debug, Deserialize)]
                    struct GoneResponse {
                        message: String,
                    }

                    let body: GoneResponse = response.json().await?;
                    error!("Export failed: {body:?}");

                    return Err(ExporterError::ExportFail {
                        reason: body.message,
                    });
                }
                _ => {
                    return Err(ExporterError::Other {
                        status: response.status(),
                    })
                }
            }
        }
    }

    async fn run_exporter<T: Serialize>(
        client: &PretixClient,
        organizer: &OrganizerId,
        event: &EventId,
        exporter_identifier: &str,
        parameters: Option<T>,
    ) -> Result<String> {
        debug!("Running exporter {organizer}/{event}/{exporter_identifier}/");

        #[derive(Deserialize)]
        struct ResponseBody {
            download: String,
        }

        let mut builder = client.post(client.url(format!(
            "/api/v1/organizers/{organizer}/events/{event}/exporters/{exporter_identifier}/run/"
        )));

        if let Some(params) = parameters {
            builder = builder.json(&params);
        }

        let response: ResponseBody = builder.send().await?.error_for_status()?.json().await?;

        Ok(response.download)
    }
}
