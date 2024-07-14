use std::fmt::Debug;
use std::fs;
use std::io::BufReader;
use std::path::Path;
use actix_cors::Cors;
use actix_web::{App, HttpResponse, HttpServer, web};
use noiseless_tracing_actix_web::NoiselessRootSpanBuilder;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig};
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::mpsc::Sender;
use tracing::{info, instrument};
use tracing_actix_web::TracingLogger;

use crate::config::WebServer;

pub struct LoginServer;

#[derive(Debug, Error)]
pub enum LoginServerError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("No private key exists in file")]
    NoPrivateKey,
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
    #[instrument]
    async fn load_certs<P: AsRef<Path> + Debug>(ssl_key: P, ssl_cert: P) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), LoginServerError> {
        let mut pem = BufReader::new(fs::File::open(ssl_cert.as_ref())?);
        let cert = rustls_pemfile::certs(&mut pem).collect::<Result<Vec<_>, _>>()?;

        let mut privkey = BufReader::new(fs::File::open(ssl_key.as_ref())?);
        let private_key = rustls_pemfile::private_key(&mut privkey)?
            .ok_or(LoginServerError::NoPrivateKey)?;

        Ok((cert, private_key))
    }

    /// Start an HTTP server and wait for an OAuth2 callback.
    #[instrument(skip(config))]
    pub async fn wait_for_callback(config: &WebServer) -> Result<CallbackResult, LoginServerError> {
        // We use a mpsc channel rather than an oneshot channel as actix wants to be multithreaded,
        // but we use it single threaded here. It's an oneshot server basically :)
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        // Setup SSL certificates. OAuth2 requires HTTPS
        let (cert, privkey) = Self::load_certs(&config.ssl_key, &config.ssl_cert).await?;
        let ssl_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert, privkey)?;

        // Start an HTTP server for oauth callback.
        // Run it on a different thread as the server's eventloop would
        // hang our flow.
        let (handle_tx, mut handle_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let server = HttpServer::new(move || App::new()
                .wrap(Cors::permissive())
                .wrap(TracingLogger::<NoiselessRootSpanBuilder>::new())
                .app_data(web::Data::new(tx.clone()))
                .route("/callback", web::get().to(Self::handle_callback))
                .route("/", web::get().to(alive))
            )
                .workers(1)
                .shutdown_timeout(5)
                .bind_rustls_0_23("0.0.0.0:443", ssl_config)
                .expect("Binding to port 443")
                .run();
            let _ = handle_tx.send(server.handle());
            server.await
        });

        let handle = loop {
            if let Ok(handle) = handle_rx.try_recv() {
                break handle;
            }
        };

        info!("Started web server, waiting for callback");

        // Wait for the callback result to come in
        let callback_result = loop {
            if let Ok(value) = rx.try_recv() {
                break value;
            }
        };

        info!("Callback received. Stopping embedded HTTP server");

        // Stop the HTTP server, don't need it anymore
        handle.stop(false).await;

        info!("HTTP server stopped");

        Ok(callback_result)
    }

    /// Handler for the OAuth2 callback from the user.
    /// Once the callback comes in, it is sent over the provided mpsc channel.
    #[instrument(skip_all)]
    async fn handle_callback(tx: web::Data<Sender<CallbackResult>>, query: web::Query<CallbackResult>) -> String {
        let _ = tx.send(query.into_inner()).await;
        "OK. You can close this page now.".to_string()
    }
}

async fn alive() -> HttpResponse {
    HttpResponse::Ok().body("Yup, alive")
}