use serde::{Deserialize, Serialize};

pub fn login_url<S1, S2, S3>(
    client_id: S1,
    redirect_uri: S2,
    pretix_uri: S3,
) -> String where
    S1: AsRef<str>,
    S2: AsRef<str>,
    S3: AsRef<str>,
{
    format!(
        "{}/api/v1/oauth/authorize?client_id={}&response_type=code&scope=read+write&redirect_uri={}",
        pretix_uri.as_ref(),
        client_id.as_ref(),
        redirect_uri.as_ref(),
    )
}

#[derive(Deserialize)]
pub struct OAuthTokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

pub async fn exchange_code<S1, S2, S3, S4>(
    code: String,
    client_id: S1,
    client_secret: S2,
    redirect_uri: S3,
    pretix_uri: S4,
) -> Result<OAuthTokenPair, reqwest::Error> where
    S1: AsRef<str>,
    S2: AsRef<str>,
    S3: AsRef<str>,
    S4: AsRef<str>,
{
    #[derive(Serialize)]
    struct RequestForm<'a> {
        redirect_uri: &'a str,
        grant_type: &'a str,
        code: &'a str,
    }

    Ok(reqwest::Client::new()
        .post(&format!("{}/api/v1/oauth/token", pretix_uri.as_ref()))
        .basic_auth(
            client_id.as_ref(),
            Some(client_secret.as_ref())
        )
        .form(&RequestForm {
            code: &code,
            grant_type: "authorization_code",
            redirect_uri: redirect_uri.as_ref(),
        })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?
    )
}