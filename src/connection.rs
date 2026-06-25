use anyhow::{anyhow, Context, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{timeout, Duration},
};

use crate::request::{Method, Request};
use crate::response::Response;
use crate::router;

const MAX_REQUEST_SIZE: usize = 8 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const CHUNK_SIZE: usize = 1024;

pub async fn handle(mut socket: TcpStream) -> Result<()> {
    let mut keep_alive = true;

    while keep_alive {
        let request_bytes = match read_request(&mut socket).await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                println!("Cliente cerró la conexión");
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error leyendo petición: {e}");
                let response = Response::bad_request();
                let _ = send_response(
                    &mut socket,
                    response,
                    "close",
                    None,
                    "respuesta 400",
                )
                .await;
                return Ok(());
            }
        };

        let request = match Request::new(&request_bytes) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("Petición malformada: {e}");
                let response = Response::bad_request();
                send_response(&mut socket, response, "close", None, "respuesta 400")
                    .await?;
                return Ok(());
            }
        };

        keep_alive = request.wants_keep_alive();
        let connection_header = if keep_alive { "keep-alive" } else { "close" };
        let encoding = if request.accepts_encoding("gzip") {
            Some("gzip")
        } else {
            None
        };

        let mut response = router::route(&request).await;
        if request.method() == Method::Head {
            response = response.head();
        }

        send_response(
            &mut socket,
            response,
            connection_header,
            encoding,
            "respuesta",
        )
        .await?;
    }

    Ok(())
}

async fn send_response(
    socket: &mut TcpStream,
    response: Response,
    connection: &str,
    encoding: Option<&str>,
    context: &str,
) -> Result<()> {
    let bytes = response.into_http_bytes(connection, encoding).unwrap_or_else(|e| {
        eprintln!("fallo al serializar respuesta: {e}");
        Response::internal_server_error()
            .into_http_bytes("close", None)
            .unwrap_or_default()
    });

    socket
        .write_all(&bytes)
        .await
        .with_context(|| format!("fallo al escribir {context}"))?;
    socket
        .flush()
        .await
        .with_context(|| format!("fallo al hacer flush {context}"))?;
    Ok(())
}

async fn read_request(socket: &mut TcpStream) -> Result<Option<Vec<u8>>> {
    let mut buffer = Vec::with_capacity(CHUNK_SIZE);
    let mut temp = [0u8; CHUNK_SIZE];

    loop {
        let n = timeout(READ_TIMEOUT, socket.read(&mut temp))
            .await
            .with_context(|| "timeout leyendo del socket")??;

        if n == 0 {
            if buffer.is_empty() {
                // El cliente cerró la conexión de forma limpia.
                return Ok(None);
            }
            return Err(anyhow!(
                "Cliente cerró la conexión antes de enviar una petición completa"
            ));
        }

        if buffer.len() + n > MAX_REQUEST_SIZE {
            return Err(anyhow!("Petición demasiado grande"));
        }

        buffer.extend_from_slice(&temp[..n]);

        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(Some(buffer));
        }
    }
}
