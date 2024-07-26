use crate::args::{ProgramArgs, WeekelijksePlezierArgs};
use crate::config::Config;
use crate::modes::{ExternalClients, Mode};
use color_eyre::eyre::Error;
use color_eyre::Result;
use exact_request::api::sales_entry::{get_sales_entry_for_entry_number, get_sales_entry_lines};
use pretix::pretix_totals;
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
        _program_args: &ProgramArgs,
        _config: &Config,
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
            get_sales_entry_for_entry_number(&exact_client, args.transaction_id).await?;
        let sales_entry_lines = get_sales_entry_lines(&exact_client, &sales_entry).await?;
        info!("{sales_entry_lines:?}");

        let offset = UtcOffset::from_whole_seconds(args.utc_offset_hours * 3600)?;

        info!("Running Pretix exports");
        let summaries = pretix_totals(
            &pretix_client,
            last_monday(offset) - Duration::weeks(args.periods_ago as i64),
            offset,
        )
        .await?;
        info!("Pretix exports complete");

        for (key, summary) in &summaries {
            info!("Event {}: {:?}", key, summary.totals);
        }

        // TODO insert into Exact

        Ok(())
    }
}
