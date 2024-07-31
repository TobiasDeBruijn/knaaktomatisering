use crate::args::{ExecutionMode, ProgramArgs};
use crate::auth::ensure_authentication;
use crate::config::{Config, Credentials, OAuthTokenPair};
use crate::modes::weekelijkse_plezier::WeekelijksePlezier;
use crate::modes::{ExternalClients, Mode};
use clap::Parser;
use color_eyre::eyre::Error;
use exact_request::api::me::accounting_division;
use exact_request::ExactClient;
use pretix_request::PretixClient;
use std::str::FromStr;
use tracing::info;
use tracing_error::ErrorLayer;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{registry, EnvFilter};

mod args;
mod auth;
mod config;
mod modes;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    // Parse command line
    let prog_args = ProgramArgs::parse();
    // Parse config file
    let mut config = Config::read(&prog_args.config).await?;

    install_tracing(&config.log)?;
    info!(
        "{} v{} by {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_AUTHORS")
    );
    info!("De centjesautomaat van Sticky");

    // Required for some SSL stuff. Must be done at most once
    // per program, so why not do it right at the start.
    init_rustls()?;

    // Check authentication & update config with new tokens
    ensure_authentication(&mut config).await?;
    config.write(&prog_args.config).await?;

    // We have this flag because you often
    // bind to port 443 (and that's the default behaviour),
    // which either requires sudo or a CAP.
    // Adding this flag allows the user to do all authorization
    // work as sudo, but run the rest as a normal user.
    if prog_args.only_auth {
        info!("Flag '--only-auth' set. Stopping here");
        return Ok(());
    }

    // Initialize all required external clients like
    // Exact Online and Pretix.
    let clients = init_external_clients(&config).await?;

    // Run the program in the desired mode.
    match &prog_args.mode.as_ref().expect("No execution mode provided") {
        ExecutionMode::WeekelijksePlezier(args) => {
            WeekelijksePlezier::execute_mode(args, &prog_args, &config, &clients).await
        }
    }?;

    Ok(())
}

/// Initialize all external clients.
/// Requires all clients have a valid access token configured.
///
/// # Errors
///
/// If a client could not be initialized
async fn init_external_clients(config: &Config) -> color_eyre::Result<ExternalClients> {
    let pretix_client = pretix_client(config);

    let mut exact_client = exact_client(config);
    // We need to query the account division, we use this is in all subsequent requests.
    exact_client.set_division(accounting_division(&exact_client).await?);

    Ok(ExternalClients {
        pretix: pretix_client,
        exact: exact_client,
    })
}

/// Create an Exact client.
/// Requires the access token to be set.
fn exact_client(config: &Config) -> ExactClient {
    ExactClient::new(access_token(config, |c| &c.exact))
}

/// Create a pretix client.
/// Requires the access token to be set.
fn pretix_client(config: &Config) -> PretixClient {
    PretixClient::new(
        access_token(config, |c| &c.pretix),
        config.pretix.url.clone(),
    )
}

/// Retrieve an application's access token from the configuration file.
///
/// # Panics
///
/// If the credentials section is `None` or the returned Option from `f` is `None`.
fn access_token<F>(config: &Config, f: F) -> String
where
    F: Fn(&Credentials) -> &Option<OAuthTokenPair>,
{
    let credentials = &config.credentials.as_ref().unwrap();

    let app_credentials = f(*credentials);

    app_credentials.as_ref().unwrap().access_token.clone()
}

/// Initialize the rustls crypto provider.
/// Must be called once in the program
fn init_rustls() -> color_eyre::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| Error::msg("Failed to initialize Rustls crypto provider"))?;
    Ok(())
}

/// Install the tracing subscriber using the directive provided
/// as per [EnvFilter::from_str].
///
/// # Errors
///
/// - If an invalid directive is provided
/// - If the subscriber could not be initialized, for example, if a logger is already initialized.
fn install_tracing<S: AsRef<str>>(directive: S) -> color_eyre::Result<()> {
    registry()
        .with(EnvFilter::from_str(directive.as_ref())?)
        .with(layer())
        .with(ErrorLayer::default())
        .try_init()?;
    Ok(())
}
