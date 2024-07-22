use crate::args::{Args, Mode};
use crate::config::{Config, Credentials, OAuthTokenPair};
use clap::Parser;
use color_eyre::eyre::Error;
use exact_request::me::accounting_division;
use exact_request::ExactClient;
use futures_util::future::try_join_all;
use pretix_request::data_exporter::{DataExporter, OrderDataExportOrderItem};
use pretix_request::events::Event;
use pretix_request::organizer::Organizer;
use pretix_request::PretixClient;
use std::collections::HashMap;
use std::num::ParseFloatError;
use std::str::FromStr;
use time::{Duration, OffsetDateTime, Time, UtcOffset, Weekday};
use tracing::{debug, info};
use tracing_error::ErrorLayer;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{registry, EnvFilter};

mod args;
mod config;
mod web_server;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    let mut config = Config::read(&args.config).await?;

    install_tracing(&config.log)?;
    info!(
        "{} v{} by {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_AUTHORS")
    );
    info!("De centjesautomaat van Sticky");
    setup_rustls()?;

    // Check authentication & update config with new tokens
    ensure_authentication(&mut config).await?;
    config.write(&args.config).await?;

    if args.only_auth {
        info!("Flag '--only-auth' set. Stopping here");
        return Ok(());
    }

    let pretix_client = PretixClient::new(
        &config
            .credentials
            .as_ref()
            .unwrap()
            .pretix
            .as_ref()
            .unwrap()
            .access_token,
        config.pretix.url,
    );
    match args.mode {
        Mode::WeekelijksePlezier {
            periods_ago,
            transaction_id: _transaction_id,
            utc_offset_hours,
        } => {
            let offset = UtcOffset::from_whole_seconds(utc_offset_hours * 3600)?;

            info!("Running Pretix exports");
            let summaries = pretix_totals(
                &pretix_client,
                last_monday(offset) - Duration::weeks(periods_ago as i64),
                offset,
            )
            .await?;
            info!("Pretix exports complete");

            for (key, summary) in &summaries {
                info!("Event {}: {:?}", key, summary.totals);
            }

            // TODO insert into Exact
        }
    }

    Ok(())
}

/// Determine the last monday.
/// If today is monday, returns today.
/// If today is tuesday, returns yesterday.
/// Time is set to midnight.
fn last_monday(offset: UtcOffset) -> OffsetDateTime {
    let now = OffsetDateTime::now_utc().to_offset(offset);

    let date = now.date();
    let last_monday = match now.weekday() {
        Weekday::Monday => date,
        Weekday::Tuesday => date - Duration::days(1),
        Weekday::Wednesday => date - Duration::days(2),
        Weekday::Thursday => date - Duration::days(3),
        Weekday::Friday => date - Duration::days(4),
        Weekday::Saturday => date - Duration::days(5),
        Weekday::Sunday => date - Duration::days(6),
    };

    last_monday.midnight().assume_offset(offset)
}

struct EventSummary {
    totals: OrderExportTotals,
    pdf: Vec<u8>,
}

/// Run a Pretix export for all available events for the export period.
async fn pretix_totals(
    pretix_client: &PretixClient,
    export_period_start: OffsetDateTime,
    offset: UtcOffset,
) -> color_eyre::Result<HashMap<String, EventSummary>> {
    let (period_start, period_end) = pretix_export_period(export_period_start, offset)?;

    // List all organizers we have access to,
    // within each organizer, list all events,
    // for each event, run an export and compute the totals
    let results = try_join_all(Organizer::list(&pretix_client).await?.into_iter().map(
        |organizer| async move {
            try_join_all(
                Event::list(&pretix_client, &organizer.slug)
                    .await?
                    .into_iter()
                    // We do not need to check closed events
                    .filter(|event| event.live)
                    .map(|event| {
                        let organizer_id = &organizer.slug;
                        async move {
                            // Get all orders of this event
                            let data_export = DataExporter::export_order_data(
                                pretix_client,
                                organizer_id,
                                &event.slug,
                            )
                            .await?;

                            // Keep only those within our export period
                            let data_export = data_export
                                .into_iter()
                                .filter(|order_item| {
                                    order_item.datetime >= period_start
                                        && order_item.datetime <= period_end
                                })
                                .collect::<Vec<_>>();

                            // Compute totals
                            let totals = order_export_calc_totals(&data_export)?;

                            let pdf = DataExporter::export_order_data_pdf(
                                &pretix_client,
                                organizer_id,
                                &event.slug,
                                period_start.clone(),
                                period_end.clone(),
                            )
                            .await?;

                            let key = event
                                .name
                                .get("en")
                                .map(|name| name.clone())
                                .unwrap_or(event.slug.to_string().clone());

                            Ok::<_, Error>((key, EventSummary { totals, pdf }))
                        }
                    }),
            )
            .await
        },
    ))
    .await?
    .into_iter()
    .flatten()
    .collect::<HashMap<_, _>>();

    Ok(results)
}

/// Return the start and end dates for the Pretix export.
/// The provided `monday` indicates the start of the export period,
/// the end date will be the first sunday following the provided monday.
///
/// The time of both dates will be midnight.
///
/// # Errors
///
/// If `monday` is not actually a monday.
fn pretix_export_period(
    monday: OffsetDateTime,
    offset: UtcOffset,
) -> color_eyre::Result<(OffsetDateTime, OffsetDateTime)> {
    if monday.weekday().ne(&Weekday::Monday) {
        return Err(Error::msg(format!(
            "The 'Monday' provided is not actually a monday, but a {}",
            monday.weekday()
        )));
    }

    Ok((
        monday
            .date()
            .with_time(Time::MIDNIGHT)
            .assume_offset(offset),
        (monday.date() + Duration::days(6))
            .with_time(Time::MIDNIGHT)
            .assume_offset(offset),
    ))
}

#[derive(Debug)]
struct OrderExportTotals {
    /// The total amount without VAT or fees
    value: f32,
    /// The total fees without VAT or fees
    fees: f32,
}

/// Calculate the totals for the provided set of order items.
///
/// # Errors
///
/// If the order items contains string which cannot be parsed to floats
fn order_export_calc_totals(
    items: &[OrderDataExportOrderItem],
) -> Result<OrderExportTotals, ParseFloatError> {
    let (value, fees) = items
        .iter()
        .map(|item| {
            let value = f32::from_str(&item.total)?;
            let fees = item
                .fees
                .iter()
                .map(|fee| f32::from_str(&fee.value))
                .collect::<Result<Vec<_>, ParseFloatError>>()?
                .into_iter()
                .sum::<f32>();

            Ok((value - fees, fees))
        })
        .collect::<Result<Vec<(f32, f32)>, ParseFloatError>>()?
        .into_iter()
        .fold((0f32, 0f32), |(acc_value, acc_fee), (value, fee)| {
            (acc_value + value, acc_fee + fee)
        });

    Ok(OrderExportTotals { value, fees })
}

/// Initialize the rustls crypto provider.
/// Must be called once in the program
fn setup_rustls() -> color_eyre::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| Error::msg("Failed to initialize Rustls crypto provider"))?;
    Ok(())
}

/// Ensure all required services have a working access token
async fn ensure_authentication(config: &mut Config) -> color_eyre::Result<()> {
    info!("Checking authorizations");

    ensure_exact_authentication(config).await?;
    ensure_pretix_authentication(config).await?;

    info!("All authorizations are present");
    Ok(())
}

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
async fn ensure_pretix_authentication(config: &mut Config) -> color_eyre::Result<()> {
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
async fn ensure_exact_authentication(config: &mut Config) -> color_eyre::Result<()> {
    if !is_exact_authorized(config).await? {
        info!("No Exact Online token pair available. Need to authorize.");
        let login_url = exact_request::oauth::login_url(
            &config.exact.oauth.client_id,
            &config.exact.oauth.redirect_uri,
        );

        info!("Please open the following URL and log in: {login_url}");

        // Wait for the login callback
        let callback_result =
            web_server::LoginServer::wait_for_callback(&config.web_server).await?;
        info!("Received login callback");

        // Exchange the callback result for a token pair
        let token_pair = exact_request::oauth::exchange_code(
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

/// Install the tracing subscriber
fn install_tracing<S: AsRef<str>>(directive: S) -> color_eyre::Result<()> {
    registry()
        .with(EnvFilter::from_str(directive.as_ref())?)
        .with(layer())
        .with(ErrorLayer::default())
        .try_init()?;
    Ok(())
}
