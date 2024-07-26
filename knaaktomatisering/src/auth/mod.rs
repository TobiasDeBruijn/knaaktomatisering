mod exact;
mod pretix;
mod web_server;

use crate::auth::exact::ensure_exact_authentication;
use crate::auth::pretix::ensure_pretix_authentication;
use crate::config::Config;
use tracing::info;

/// Ensure all required services have a working access token
pub async fn ensure_authentication(config: &mut Config) -> color_eyre::Result<()> {
    info!("Checking authorizations");

    ensure_exact_authentication(config).await?;
    ensure_pretix_authentication(config).await?;

    info!("All authorizations are present");
    Ok(())
}
