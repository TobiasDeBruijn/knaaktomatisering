use crate::{ExactClient, ExactError, ExactPayload};
use exact_filter::{Filter, FilterOp, Guid};
use serde::Deserialize;

pub async fn get_sales_entry_for_entry_number(
    client: &ExactClient,
    number: i32,
) -> Result<Guid, ExactError> {
    #[derive(Deserialize)]
    struct Response {
        #[serde(rename = "EntryID")]
        entry_id: Guid,
    }

    let response: ExactPayload<Response> = client
        .get(client.divisioned_url(format!(
            "/salesentry/SalesEntries?$filter=EntryNumber+eq+{number}&$select=EntryID"
        ))?)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(response.value().entry_id)
}

#[derive(Debug, Deserialize)]
pub struct SalesEntryLine {
    #[serde(rename = "ID")]
    id: Guid,
    /// The value of the line excluding VAT
    #[serde(rename = "AmountFC")]
    amount_fc: f32,
    #[serde(rename = "VATCode")]
    vat_code: String,
    #[serde(rename = "VATPercentage")]
    vat_percentage: f32,
    #[serde(rename = "CostCenter")]
    cost_center: Option<String>,
    #[serde(rename = "Description")]
    description: String,
}

pub async fn get_sales_entry_lines(
    client: &ExactClient,
    entry_id: &Guid,
) -> Result<Vec<SalesEntryLine>, ExactError> {
    let response: ExactPayload<SalesEntryLine> = client.get(client.divisioned_url(
            format!("/salesentry/SalesEntryLines?$select=ID,AmountFC,VATCode,VATPercentage,CostCenter,Description&$filter={}",
                Filter::new("EntryID", entry_id, FilterOp::Equals).finalize()
            )
        )?)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(response.values())
}
