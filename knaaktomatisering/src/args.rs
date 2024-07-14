use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct Args {
    /// Path to the JSON configuration file
    #[clap(long, short)]
    pub config: PathBuf,

    #[clap(subcommand)]
    pub mode: Mode,
}

#[derive(Debug, Subcommand)]
pub enum Mode {
    /// The weekly fun of Mollie, Koala and Pretix.
    /// Applies Pretix payments to the Koala export.
    ///
    /// Requires the Koala export to be already imported into Exact. This will then add
    /// all Pretix lines to the sale transaction
    WeekelijksePlezier {
        /// The ref of the sale transaction created by importing
        /// the Koala export.
        #[clap(long, short)]
        transaction_id: i32,
        /// How many periods ago to add to the sale order.
        /// A value of 1 indicates the most recent finished period.
        /// This would mean, from two sundays ago to the most recent monday (Koala is inclusive),
        /// for pretix it is up to the most recent sunday (Pretix is exclusive).
        #[clap(long, short, default_value_t = 1)]
        periods_ago: i32,
    },
}
