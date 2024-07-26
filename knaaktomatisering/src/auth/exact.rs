use crate::auth::web_server;
use crate::config::{Config, Credentials, OAuthTokenPair};
use exact_request::api::me::accounting_division;
use exact_request::ExactClient;
use tracing::{debug, info};

/// Check that Exact credentials exist and that they work
async fn is_exact_authorized(config: &Config) -> color_eyre::Result<bool> {
    Ok(
        if let Some(Some(exact_credentials)) = config
            .credentials
            .as_ref()
            .map(|credentials| &credentials.exact)
        {
            debug!("Checking if Exact credentials still work");

            let client = ExactClient::new(&exact_credentials.access_token);
            match accounting_division(&client).await {
                Ok(_) => true,
                Err(e) => match e.status() {
                    Some(http::StatusCode::UNAUTHORIZED) => {
                        info!("Exact Online credentials present, but no longer valid");
                        false
                    }
                    _ => return Err(e.into()),
                },
            }
        } else {
            false
        },
    )
}

/// Ensure that there is a valid Exact access token.
/// Asks the user to log in if that is not currently the case.
pub async fn ensure_exact_authentication(config: &mut Config) -> color_eyre::Result<()> {
    if !is_exact_authorized(config).await? {
        info!("No Exact Online token pair available. Need to authorize.");
        let login_url = exact_request::api::oauth::login_url(
            &config.exact.oauth.client_id,
            &config.exact.oauth.redirect_uri,
        );

        info!("Please open the following URL and log in: {login_url}");

        // Wait for the login callback
        let callback_result =
            web_server::LoginServer::wait_for_callback(&config.web_server).await?;
        info!("Received login callback");

        // Exchange the callback result for a token pair
        let token_pair = exact_request::api::oauth::exchange_code(
            callback_result.code,
            &config.exact.oauth.client_id,
            &config.exact.oauth.client_secret,
            &config.exact.oauth.redirect_uri,
        )
        .await?;

        info!("Exact Online login successful");

        // Update the configuration
        if let Some(credentials) = &mut config.credentials {
            credentials.exact = Some(OAuthTokenPair {
                access_token: token_pair.access_token.clone(),
                refresh_token: token_pair.refresh_token.clone(),
            });
        } else {
            config.credentials = Some(Credentials {
                exact: Some(OAuthTokenPair {
                    access_token: token_pair.access_token,
                    refresh_token: token_pair.refresh_token,
                }),
                pretix: None,
            });
        }
    }

    Ok(())
}
