use std::str::FromStr;
use clap::Parser;
use tracing::info;
use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, registry};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::args::Args;
use crate::config::{Config, Credentials, OAuthTokenPair};

mod config;
mod args;
mod web_server;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    let mut config = Config::read(&args.config).await?;

    install_tracing(&config.log)?;
    info!("{} v{} by {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS"));
    info!("De centjesautomaat van Sticky");

    let have_exact = config.credentials
        .as_ref()
        .map(|v| v.exact.is_some())
        .unwrap_or(false);
    let have_pretix = config.credentials
        .as_ref()
        .map(|v| v.pretix.is_some())
        .unwrap_or(false);

    // Login with Exact if needed
    if !have_exact {
        info!("No Exact Online token pair available. Need to authorize.");
        let login_url = exact_request::oauth::login_url(
            &config.exact.oauth.client_id,
            &config.exact.oauth.redirect_uri,
        );

        info!("Please open the following URL and log in: {login_url}");

        // Wait for the login callback
        let callback_result = web_server::LoginServer::wait_for_callback(&config.web_server).await?;
        info!("Received login callback");

        // Exchange the callback result for a token pair
        let token_pair = exact_request::oauth::exchange_code(
            callback_result.code,
            &config.exact.oauth.client_id,
            &config.exact.oauth.client_secret,
            &config.exact.oauth.redirect_uri,
        ).await?;

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
                pretix: None
            });
        }
    }

    // Login with Pretix if needed
    if !have_pretix {
        info!("No Pretix token pair available. Need to authorize.");
        let login_url = pretix_request::oauth::login_url(
            &config.exact.oauth.client_id,
            &config.exact.oauth.redirect_uri,
            &config.pretix.url,
        );

        info!("Please open the following URL and log in: {login_url}");

        // Wait for the login callback
        let callback_result = web_server::LoginServer::wait_for_callback(&config.web_server).await?;

        // Exchange the callbackr result for a token pair
        let token_pair = pretix_request::oauth::exchange_code(
            callback_result.code,
            &config.exact.oauth.client_id,
            &config.exact.oauth.client_secret,
            &config.exact.oauth.redirect_uri,
            &config.pretix.url
        ).await?;

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

    config.write(&args.config).await?;

    // TODO write tokens to configuration


    Ok(())
}

/// Install the tracing subscriber
fn install_tracing<S: AsRef<str>>(directive: S) -> color_eyre::Result<()> {
    registry()
        .with(EnvFilter::from_str(directive.as_ref())?)
        .with(ErrorLayer::default())
        .try_init()?;
    Ok(())
}