use crate::auth::web_server;
use crate::config::{Config, Credentials, OAuthTokenPair};
use pretix_request::organizer::Organizer;
use pretix_request::PretixClient;
use tracing::{debug, info};

/// Check that Pretix credentials exist and that they work
async fn is_pretix_authorized(config: &Config) -> color_eyre::Result<bool> {
    Ok(
        if let Some(Some(pretix_credentials)) = &config
            .credentials
            .as_ref()
            .map(|credentials| &credentials.pretix)
        {
            debug!("Checking if Pretix credentials still work");

            let client =
                PretixClient::new(&pretix_credentials.access_token, config.pretix.url.clone());
            match Organizer::list(&client).await {
                Ok(_) => true,
                Err(e) => match e.status() {
                    Some(http::StatusCode::UNAUTHORIZED) => {
                        info!("Pretix credentials present, but no longer valid");
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

/// Ensure that there is a valid Pretix access token.
/// Asks the user to log in if that is not currently the case.
pub async fn ensure_pretix_authentication(config: &mut Config) -> color_eyre::Result<()> {
    // Login with Pretix if needed
    if !is_pretix_authorized(config).await? {
        info!("No Pretix token pair available. Need to authorize.");
        let login_url = pretix_request::oauth::login_url(
            &config.pretix.oauth.client_id,
            &config.pretix.oauth.redirect_uri,
            &config.pretix.url,
        );

        info!("Please open the following URL and log in: {login_url}");

        // Wait for the login callback
        let callback_result =
            web_server::LoginServer::wait_for_callback(&config.web_server).await?;
        info!("Received callback");

        // Exchange the callbackr result for a token pair
        let token_pair = pretix_request::oauth::exchange_code(
            callback_result.code,
            &config.pretix.oauth.client_id,
            &config.pretix.oauth.client_secret,
            &config.pretix.oauth.redirect_uri,
            &config.pretix.url,
        )
        .await?;

        info!("Login with Pretix successful");

        // Update the configuration
        if let Some(credentials) = &mut config.credentials {
            credentials.pretix = Some(OAuthTokenPair {
                access_token: token_pair.access_token.clone(),
                refresh_token: token_pair.refresh_token.clone(),
            });
        } else {
            config.credentials = Some(Credentials {
                exact: None,
                pretix: Some(OAuthTokenPair {
                    access_token: token_pair.access_token,
                    refresh_token: token_pair.refresh_token,
                }),
            });
        }
    }

    Ok(())
}
