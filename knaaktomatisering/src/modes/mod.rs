use crate::args::ProgramArgs;
use crate::config::Config;
use color_eyre::Result;
use exact_request::ExactClient;
use pretix_request::PretixClient;

pub mod weekelijkse_plezier;

pub trait Mode {
    type Args;

    async fn execute_mode(
        args: &Self::Args,
        program_args: &ProgramArgs,
        config: &Config,
        external_clients: &ExternalClients,
    ) -> Result<()>;
}

pub struct ExternalClients {
    pub exact: ExactClient,
    pub pretix: PretixClient,
}
