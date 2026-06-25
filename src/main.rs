mod connection;
mod handlers;
mod request;
mod response;
mod router;
mod static_files;

use std::env;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio::sync::{Notify, Semaphore};

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "8080";
const MAX_CONNECTIONS: usize = 10_000;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let host = env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port = env::var("PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let addr = format!("{host}:{port}");

    let tcp_listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("fallo al vincular servidor TCP en {addr}"))?;
    tracing::info!("Servidor escuchando en {addr}");

    let semaphore = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let active = Arc::new(AtomicUsize::new(0));
    let shutdown = Arc::new(Notify::new());

    let server_loop = async {
        loop {
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .context("fallo al adquirir permiso de conexión")?;
            let (socket, peer) = tcp_listener.accept().await?;

            active.fetch_add(1, Ordering::SeqCst);
            let active = active.clone();
            let shutdown = shutdown.clone();

            tokio::spawn(async move {
                let _permit = permit;
                if let Err(e) = connection::handle(socket).await {
                    tracing::error!(peer = %peer, "Error en conexión: {e}");
                }
                if active.fetch_sub(1, Ordering::SeqCst) == 1 {
                    shutdown.notify_one();
                }
            });
        }

        #[allow(unreachable_code)]
        Result::<(), anyhow::Error>::Ok(())
    };

    tokio::select! {
        result = server_loop => result?,
        () = wait_for_shutdown() => {
            tracing::info!("señal de shutdown recibida, deteniendo servidor");
        }
    }

    // Esperar a que no haya conexiones activas (con timeout de 30s).
    if active.load(Ordering::SeqCst) > 0 {
        if tokio::time::timeout(tokio::time::Duration::from_secs(30), shutdown.notified())
            .await
            .is_ok()
        {
            tracing::info!("todas las conexiones activas finalizaron");
        } else {
            tracing::warn!("timeout esperando conexiones activas");
        }
    }

    tracing::info!("servidor detenido");
    Ok(())
}

async fn wait_for_shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("fallo al registrar SIGTERM");
        let mut sigint = signal(SignalKind::interrupt()).expect("fallo al registrar SIGINT");

        tokio::select! {
            _ = sigterm.recv() => tracing::info!("SIGTERM recibida"),
            _ = sigint.recv() => tracing::info!("SIGINT recibida"),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("fallo al registrar Ctrl+C");
        tracing::info!("Ctrl+C recibido");
    }
}
