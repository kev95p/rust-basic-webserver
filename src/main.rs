mod connection;
mod handlers;
mod request;
mod response;
mod router;
mod static_files;

use std::env;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "8080";
const MAX_CONNECTIONS: usize = 10_000;

#[tokio::main]
async fn main() -> Result<()> {
    let host = env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port = env::var("PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let addr = format!("{host}:{port}");

    let tcp_listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("fallo al vincular servidor TCP en {addr}"))?;
    println!("Servidor escuchando en {addr}");

    let semaphore = std::sync::Arc::new(Semaphore::new(MAX_CONNECTIONS));

    loop {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .context("fallo al adquirir permiso de conexión")?;
        let (socket, peer) = tcp_listener.accept().await?;

        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = connection::handle(socket).await {
                eprintln!("Error en conexión {peer}: {e}");
            }
        });
    }
}
