mod connection;
mod handlers;
mod request;
mod response;
mod router;
mod static_files;

use anyhow::{Context, Result};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    let tcp_listener = TcpListener::bind("0.0.0.0:8080")
        .await
        .context("fallo al vincular servidor TCP")?;
    println!("Servidor escuchando en 0.0.0.0:8080");

    loop {
        let (mut socket, _) = tcp_listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = connection::handle(&mut socket).await {
                eprintln!("Error en conexión: {e}");
            }
        });
    }
}
