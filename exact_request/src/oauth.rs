use serde::{Deserialize, Serialize};

pub fn login_url<S1, S2>(
    client_id: S1,
    redirect_uri: S2
) -> String where
    S1: AsRef<str>,
    S2: AsRef<str> {
    format!(
        "https://start.exactonline.nl/api/oauth2/auth?client_id={}&redirect_uri={}&response_type=code&force_login=0",
        client_id.as_ref(),
        redirect_uri.as_ref()
    )
}

#[derive(Deserialize)]
pub struct OAuthTokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

pub async fn exchange_code<S1, S2, S3>(
    code: String,
    client_id: S1,
    client_secret: S2,
    redirect_uri: S3,
) -> Result<OAuthTokenPair, reqwest::Error> where
    S1: AsRef<str>,
    S2: AsRef<str>,
    S3: AsRef<str>,
{
    #[derive(Serialize)]
    struct RequestForm<'a> {
        client_id: &'a str,
        client_secret: &'a str,
        redirect_uri: &'a str,
        grant_type: &'a str,
        code: &'a str,
    }

    Ok(reqwest::Client::new()
        .post("https://start.exactonline.nl/api/oauth2/token")
        .form(&RequestForm {
            code: &code,
            grant_type: "authorization_code",
            redirect_uri: redirect_uri.as_ref(),
            client_secret: client_secret.as_ref(),
            client_id: client_id.as_ref()
        })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?
    )
}