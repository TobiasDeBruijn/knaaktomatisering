use std::path::PathBuf;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Args {
    #[clap(long, short)]
    pub config: PathBuf,
}