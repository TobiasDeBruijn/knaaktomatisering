use crate::args::{ProgramArgs, WeekelijksePlezierArgs};
use crate::config::{Config, PretixEventId};
use crate::modes::{ExternalClients, Mode};
use color_eyre::eyre::Error;
use color_eyre::Result;
use exact_request::api::cost_center::get_cost_center_by_code;
use exact_request::api::gl_account::get_gl_account_by_code;
use exact_request::api::sales_entry::{get_sales_entry_for_entry_number, get_sales_entry_lines};
use futures_util::future::try_join_all;
use pretix::pretix_totals;
use regex::Regex;
use std::collections::HashMap;
use std::future::Future;
use time::{Duration, UtcOffset};
use time_util::last_monday;
use tracing::info;

pub mod pretix;
pub mod time_util;

pub struct WeekelijksePlezier;

impl Mode for WeekelijksePlezier {
    type Args = WeekelijksePlezierArgs;

    async fn execute_mode(
        args: &Self::Args,
        program_args: &ProgramArgs,
        config: &Config,
        external_clients: &ExternalClients,
    ) -> Result<()> {
        let exact_client = &external_clients.exact;
        let pretix_client = &external_clients.pretix;

        if args.periods_ago == 0 {
            return Err(Error::msg("Argument '--periods-ago' may not be 0. 0 would mean you're looking from the most recent monday up until the next sunday, which isn't possible, as that sunday either hasn't happened yet, or it is still sunday."));
        }

        // Fetch the sales entry to which we should import the pretix data.
        // While we don't need the data until we're going to be importing the
        // pretix data, if this fails there's no point in running the pretix
        // exports, which are expensive.
        info!("Fetching sales entry information from Exact");
        let sales_entry =
            get_sales_entry_for_entry_number(exact_client, args.transaction_id).await?;
        let sales_entry_lines = get_sales_entry_lines(exact_client, &sales_entry).await?;
        info!("{sales_entry_lines:?}");

        // Timezone hell
        let offset = UtcOffset::from_whole_seconds(args.utc_offset_hours * 3600)?;

        // Calculate the start date of the export
        let export_period_start = last_monday(offset) - Duration::weeks(args.periods_ago as i64);

        // Get the exports
        info!(
            "Running Pretix exports with start date {}",
            export_period_start
        );
        let summaries = pretix_totals(pretix_client, export_period_start, offset).await?;
        info!("Pretix exports complete");

        let do_run = !program_args.dry_run;

        for (event_key, summary) in summaries {
            info!(
                "Event {}: {:.2} with TRX {:.2}",
                event_key, summary.totals.value, summary.totals.fees
            );
            for (item_key, value) in &summary.items {
                info!("Item: {item_key} sold for {value:.2}");
            }

            // Get the event specific configuration
            let event_config = config
                .pretix
                .event_specific
                .get(&PretixEventId(event_key.to_string()))
                .ok_or(Error::msg(format!(
                    "No Event-specific configuration found for event {}",
                    event_key
                )))?;

            // Get the Exact cost center GUID for each configured cost center
            let cost_centers = try_join_all(event_config.cost_centers_per_product.iter().map(
                |(pattern, cost_center_code)| async move {
                    let cost_center_guid =
                        get_cost_center_by_code(&exact_client, cost_center_code).await?;
                    Ok::<_, Error>((
                        Regex::new(pattern.as_ref())?,
                        (cost_center_guid, cost_center_code),
                    ))
                },
            ))
            .await?;

            // GL Account used in all rows except transaction costs
            let gl_account =
                get_gl_account_by_code(&exact_client, &event_config.gl_account).await?;

            // For some events, like the introduction, the items sold should be split out in Exact.
            // For other events, like external parties, this is not the case.
            if event_config.split_per_product {
                // General line name and transaction cost line name
                let line_name = format!("Pretix {}", summary.event_name);
                let trx_line_name = format!("{line_name} | Transactiekosten");

                for (item_key, line_value) in &summary.items {
                    // Some sold items shouldn't be in Exact, like 'Algemene Introductie'.
                    // Check if we should skip the item
                    let is_ignored = event_config
                        .ignore_products
                        .iter()
                        .map(|pattern| Regex::new(pattern.as_ref()))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .any(|regex| regex.is_match(item_key));

                    if is_ignored {
                        info!(
                            "Item {}/{} is configured as ignored, ignoring.",
                            event_key, item_key
                        );
                        continue;
                    }

                    // Get the cost center associated with the itemd
                    let cost_center = &cost_centers
                        .iter()
                        .find(|(pattern, _)| pattern.is_match(&item_key))
                        .ok_or(Error::msg(format!(
                            "No matching cost center pattern found for {}/{}",
                            event_key, item_key
                        )))?
                        .1;

                    // Find the Pretix sale item corresponding with the item key
                    let sale_item = summary
                        .sale_items
                        .iter()
                        .find(|sale_item| sale_item.name.eq(item_key))
                        .ok_or(Error::msg(format!(
                            "No sale item could be found for sold item {}/{}",
                            event_key, item_key
                        )))?;

                    // Find the Exact VAT code for this item
                    let vat_code = config
                        .exact
                        .vat_codes
                        .iter()
                        .find(|code| code.percentage == sale_item.tax_rate)
                        .ok_or(Error::msg(format!(
                            "Could not find tax rate for item {}/{} with VAT percentage {}",
                            event_key, item_key, sale_item.tax_rate
                        )))?;

                    // Format the line name
                    let item_line_name = format!("{line_name} | {item_key}");

                    // Inform the user of what we will do
                    info!(
                        "Creating sale line in Exact: {} {item_line_name} {} {}% €{:.2}",
                        event_config.gl_account, cost_center.1, vat_code.percentage, line_value
                    );
                }

                info!(
                    "Creating sale line in Exact: {} {trx_line_name} €{:.2}",
                    config.exact.gl_accounts.bookkeeping, summary.totals.fees
                );
            } else {
                // Format the line name
                let line_name = format!("Pretix {}", summary.event_name);
                // Transaction cost line name
                let trx_line_name = format!("{line_name} | Transactiekosten");

                // Get the configured VAT code for this event
                let vat_code = event_config.vat_code.as_ref().ok_or(Error::msg(format!(
                    "Missing VAT code for event {event_key}"
                )))?;

                // Inform the user of what we will do
                info!(
                    "Creating sale line in Exact: {} {line_name} {vat_code}% €{:.2}",
                    event_config.gl_account, summary.totals.value
                );
                info!(
                    "Creating sale line in Exact: {} {trx_line_name} €{:.2}",
                    config.exact.gl_accounts.bookkeeping, summary.totals.fees
                );
            }
        }

        // TODO insert into Exact

        Ok(())
    }
}
