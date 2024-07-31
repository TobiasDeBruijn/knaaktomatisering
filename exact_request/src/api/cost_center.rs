use crate::{ExactClient, ExactError, ExactPayload};
use exact_filter::Guid;
use serde::Deserialize;

pub async fn get_cost_center_by_code<S: AsRef<str>>(
    client: &ExactClient,
    code: S,
) -> Result<Guid, ExactError> {
    #[derive(Deserialize)]
    struct Response {
        #[serde(rename = "ID")]
        id: Guid,
    }

    let response: ExactPayload<Response> = client
        .get(client.divisioned_url(format!(
            "/hrm/Costcenters?$filter=Code+eq+'{}'&$select=ID",
            code.as_ref()
        ))?)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(response.value().id)
}
