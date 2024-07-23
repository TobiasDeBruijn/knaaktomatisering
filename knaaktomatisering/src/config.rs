use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub log: String,
    pub web_server: WebServer,
    pub pretix: Pretix,
    pub exact: Exact,
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
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Pretix {
    pub oauth: OAuth2Config,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Exact {
    pub oauth: OAuth2Config,
    pub ledger_unassigned_payments: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebServer {
    pub ssl_cert: PathBuf,
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
    pub async fn read<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let mut f = fs::File::open(path.as_ref()).await?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).await?;

        Ok(serde_json::from_slice(&buf)?)
    }

    pub async fn write<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let buf = serde_json::to_vec_pretty(self)?;
        let mut f = fs::File::create(path.as_ref()).await?;
        f.write_all(&buf).await?;
        Ok(())
    }
}
