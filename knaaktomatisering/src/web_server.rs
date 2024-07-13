use std::fs;
use std::io::BufReader;
use std::path::Path;

use actix_cors::Cors;
use actix_web::{App, HttpServer, web};
use noiseless_tracing_actix_web::NoiselessRootSpanBuilder;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::mpsc::Sender;
use tracing_actix_web::TracingLogger;

use crate::config::WebServer;

pub struct LoginServer;

#[derive(Debug, Error)]
pub enum LoginServerError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("No private key exists in file")]
    NoPrivateKey,
    #[error("No public key exists in file")]
    NoPublicKey,
    #[error("{0}")]
    Tls(#[from] rustls::Error),
    #[error("{0}")]
    Join(#[from] tokio::task::JoinError),
}

#[derive(Deserialize)]
pub struct CallbackResult {
    pub code: String,
}

impl LoginServer {
    /// Load certificates from file and parse the PEM files.
    async fn load_certs<P: AsRef<Path>>(ssl_key: P, ssl_cert: P) -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), LoginServerError> {
        let mut pem = BufReader::new(fs::File::open(ssl_cert.as_ref())?);
        let cert = rustls_pemfile::certs(&mut pem)
            .nth(0)
            .ok_or(LoginServerError::NoPublicKey)??;

        let mut privkey = BufReader::new(fs::File::open(ssl_key.as_ref())?);
        let private_key = rustls_pemfile::private_key(&mut privkey)?
            .ok_or(LoginServerError::NoPrivateKey)?;

        Ok((cert, private_key))
    }

    /// Start an HTTP server and wait for an OAuth2 callback.
    pub async fn wait_for_callback(config: &WebServer) -> Result<CallbackResult, LoginServerError> {
        // We use a mpsc channel rather than an oneshot channel as actix wants to be multithreaded,
        // but we use it single threaded here. It's an oneshot server basically :)
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        // Setup SSL certificates. OAuth2 requires HTTPS
        let (cert, privkey) = Self::load_certs(&config.ssl_key, &config.ssl_cert).await?;
        let ssl_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], privkey)?;

        // Start an HTTP server for oauth callback.
        // Run it on a different thread as the server's eventloop would
        // hang our flow.
        let handle = tokio::spawn(async move {
            match HttpServer::new(move || App::new()
                .wrap(Cors::permissive())
                .wrap(TracingLogger::<NoiselessRootSpanBuilder>::new())
                .app_data(web::Data::new(tx.clone()))
                .route("/callback", web::get().to(Self::handle_callback))
            )
                .bind_rustls_0_23("0.0.0.0:4363", ssl_config) {
                Ok(server) => server.run(),
                Err(e) => panic!("Failed to bind web server: {e}")
            }
        });

        // Wait for the callback result to come in
        let callback_result = loop {
            if let Ok(value) = rx.try_recv() {
                break value;
            }
        };

        // Stop the HTTP server, don't need it anymore
        handle.await?.handle().stop(true).await;

        Ok(callback_result)
    }

    /// Handler for the OAuth2 callback from the user.
    /// Once the callback comes in, it is sent over the provided mpsc channel.
    async fn handle_callback(tx: web::Data<Sender<CallbackResult>>, query: web::Query<CallbackResult>) -> String {
        let _ = tx.send(query.into_inner());
        "OK. You can close this page now.".to_string()
    }
}