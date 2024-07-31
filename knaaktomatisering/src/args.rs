use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct ProgramArgs {
    /// Path to the JSON configuration file
    #[clap(long, short)]
    pub config: PathBuf,
    #[clap(subcommand)]
    pub mode: Option<ExecutionMode>,
    /// Only perform OAuth2 authorizations.
    /// Useful if you need to run the program as root for the authorizations,
    /// due to the bind to port 443, but want to run
    /// the rest of the program as a regular user.
    #[clap(long)]
    pub only_auth: bool,
    /// Only print actions that would be performed,
    /// but don't actually perform them.
    #[clap(long)]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum ExecutionMode {
    /// The weekly fun of Mollie, Koala and Pretix.
    /// Applies Pretix payments to the Koala export.
    ///
    /// Requires the Koala export to be already imported into Exact. This will then add
    /// all Pretix lines to the sale transaction
    WeekelijksePlezier(WeekelijksePlezierArgs),
}

#[derive(Debug, Args)]
pub struct WeekelijksePlezierArgs {
    /// The ref of the sale transaction created by importing
    /// the Koala export.
    #[clap(long, short)]
    pub transaction_id: i32,
    /// How many periods ago to add to the sale order.
    /// A value of 1 indicates the most recent finished period.
    /// This would mean, from two sundays ago to the most recent monday (Koala is inclusive),
    /// for pretix it is up to the most recent sunday (Pretix is exclusive).
    ///
    /// A value of `0` is not allowed
    #[clap(long, short, default_value_t = 1)]
    pub periods_ago: u32,
    /// The offset in hours with respect to UTC time.
    /// For NL this is +1 in the winter, +2 in the summer.
    ///
    /// Alternatively, you can use the `date` utility to find this export automatically.
    /// Substitute the value you'd pass to this argument with
    /// ```
    /// $(date +"%z" | cut -c 2-)
    /// ```
    #[clap(long, short)]
    pub utc_offset_hours: i32,
}
