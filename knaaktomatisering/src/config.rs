use knaaktomatisering_proc::StringLike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A Regex pattern
#[derive(Debug, Hash, PartialEq, Eq, Deserialize, Serialize, StringLike)]
pub struct RegexPattern(pub String);

/// Code for an Exact GL Account,
/// also known as 'Grootboekrekening'.
/// E.g. `1302` for unassigned payments.
#[derive(Debug, Hash, PartialEq, Eq, Deserialize, Serialize, StringLike)]
pub struct ExactGLAccountCode(pub String);

/// Code for an Exact cost center,
/// also known as `Kostenplaats`.
/// E.g. `TRX` for transaction fees.
#[derive(Debug, Hash, PartialEq, Eq, Deserialize, Serialize, StringLike)]
pub struct ExactCostCenterCode(pub String);

/// The Pretix event ID. Shown in the Pretix
/// application as 'Short form'. E.g. for
/// the introduction 2024-2025 this is
/// `intro-2024-2025`.
#[derive(Debug, Hash, PartialEq, Eq, Deserialize, Serialize, StringLike)]
pub struct PretixEventId(pub String);

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    /// Logging directive as specified by [tracing_subscriber::EnvFilter::from_str].
    pub log: String,
    /// Built-in OAuth2 web server configuration
    pub web_server: WebServer,
    /// Pretix configuration
    pub pretix: Pretix,
    /// Exact Online configuration
    pub exact: Exact,
    /// Authorized credentials.
    /// Should not be edited manually
    pub credentials: Option<Credentials>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Credentials {
    pub pretix: Option<OAuthTokenPair>,
    pub exact: Option<OAuthTokenPair>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OAuthTokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OAuth2Config {
    /// OAuth2 client ID.
    /// Generated by the application you're trying
    /// to authorize with.
    pub client_id: String,
    /// OAuth2 client secret.
    /// Generated by the application you're trying
    /// to authorize with.
    pub client_secret: String,
    /// OAuth2 redirect URI.
    /// Value should match whatever you enter
    /// when creating the OAuth2 client.
    ///
    /// The internal path used is `/callback`. If you're following
    /// the README, the URL will be `https://knaaktomatisering.local/callback`.
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Pretix {
    pub oauth: OAuth2Config,
    /// The URL of the pretix store.
    /// Last I checked this is `https://pretix.svsticky.nl`.
    /// Should *not* end with a slash (`/`).
    pub url: String,
    /// Event specific configuration
    pub event_specific: HashMap<PretixEventId, PretixEventConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PretixEventConfig {
    /// Whether items in the event should be imported to Exact
    /// as seperate order lines, rather than be combined into one.
    ///
    /// For most events, this should be `false`. Notably for
    /// the introduction this should be `true`.
    pub split_per_product: bool,
    /// The cost centers, also known as 'Kostenplaats' per product in the Pretix event.
    /// The key of this map may be a Regex pattern. The value should be an Exact
    /// cost center code. E.g. `TRX` for transaction costs.
    ///
    /// If the value for `split_per_product` is set to false, an empty map should be provided.
    pub cost_centers_per_product: HashMap<RegexPattern, ExactCostCenterCode>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Exact {
    /// OAuth configuration
    pub oauth: OAuth2Config,
    /// Exact GL accounts.
    /// Also known as 'Grootboekrekeningen'
    pub gl_accounts: ExactGlAccounts,
    /// Exact journals
    pub journals: ExactJournals,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExactJournals {
    /// The sales journal. Last I checked this is `0302`
    pub sales: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExactGlAccounts {
    /// The code for unassigned payments.
    /// Last I checked this is `1302`.
    pub unassigned_payments: ExactGLAccountCode,
    /// The code for bookkeeping.
    /// Last I checked this is `5007`.
    pub bookkeeping: ExactGLAccountCode,
    /// A mapping of a Pretix event IDs, also known as it's short form, to the code of an Exact GL Account.
    pub pretix_events: HashMap<PretixEventId, ExactGLAccountCode>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebServer {
    /// Path to SSL certificate
    pub ssl_cert: PathBuf,
    /// Path to SSL private key
    pub ssl_key: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl Config {
    /// Read the configuration from disk
    ///
    /// # Errors
    ///
    /// - IO Error
    /// - Deserialization error
    pub async fn read<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let mut f = fs::File::open(path.as_ref()).await?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).await?;

        Ok(serde_json::from_slice(&buf)?)
    }

    /// Write the current configuration to disk.
    ///
    /// # Errors
    ///
    /// - IO error
    /// - Serialization error
    pub async fn write<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let buf = serde_json::to_vec_pretty(self)?;
        let mut f = fs::File::create(path.as_ref()).await?;
        f.write_all(&buf).await?;
        Ok(())
    }
}
