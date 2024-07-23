use serde::Deserialize;

use crate::{ExactClient, ExactPayload};

pub async fn accounting_division(client: &ExactClient) -> Result<i32, reqwest::Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Response {
        accounting_division: i32,
    }

    let r: ExactPayload<Response> = client
        .get(ExactClient::url(
            "/api/v1/current/Me?$select=AccountingDivision",
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(r.value().accounting_division)
}
