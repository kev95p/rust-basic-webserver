use anyhow::{anyhow, Context, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{timeout, Duration},
};

use crate::request::Request;
use crate::response::Response;
use crate::router;

const MAX_REQUEST_SIZE: usize = 8 * 1024;

pub async fn handle(socket: &mut TcpStream) -> Result<()> {
    let mut keep_alive = true;

    while keep_alive {
        let request_bytes = match timeout(Duration::from_secs(5), read_request(socket)).await {
            Ok(Ok(Some(bytes))) => bytes,
            Ok(Ok(None)) => {
                println!("Cliente cerró la conexión");
                return Ok(());
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                println!("Timeout esperando siguiente petición");
                return Ok(());
            }
        };

        let request = match Request::new(&request_bytes) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("Petición malformada: {}", e);
                let response = Response::bad_request();
                send_response(socket, &response, "close", None, "respuesta 400").await?;
                return Ok(());
            }
        };

        println!("{:?}", request);

        keep_alive = request.wants_keep_alive();
        let connection_header = if keep_alive { "keep-alive" } else { "close" };
        let encoding = if request.accepts_encoding("gzip") {
            Some("gzip")
        } else {
            None
        };

        let response = router::route(&request);
        send_response(socket, &response, connection_header, encoding, "respuesta").await?;
    }

    Ok(())
}

async fn send_response(
    socket: &mut TcpStream,
    response: &Response,
    connection: &str,
    encoding: Option<&str>,
    context: &str,
) -> Result<()> {
    socket
        .write_all(&response.to_http_bytes(connection, encoding))
        .await
        .with_context(|| format!("fallo al escribir {}", context))?;
    socket
        .flush()
        .await
        .with_context(|| format!("fallo al hacer flush {}", context))?;
    Ok(())
}

async fn read_request(socket: &mut TcpStream) -> Result<Option<Vec<u8>>> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 1024];

    loop {
        let n = socket.read(&mut temp).await?;
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
