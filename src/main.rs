pub mod request;
pub mod response;

use anyhow::{anyhow, Context, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{timeout, Duration},
};

use crate::request::{Method, Request};
use crate::response::Response;

const MAX_REQUEST_SIZE: usize = 8 * 1024;

#[tokio::main]
async fn main() -> Result<()> {
    let tcp_listener = TcpListener::bind("0.0.0.0:8080")
        .await
        .context("fallo al vincular servidor TCP")?;
    println!("Servidor escuchando en 0.0.0.0:8080");

    loop {
        let (mut socket, _) = tcp_listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(&mut socket).await {
                eprintln!("Error en conexión: {}", e);
            }
        });
    }
}

async fn handle_connection(socket: &mut TcpStream) -> Result<()> {
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
                send_response(socket, &response, "close", "respuesta 400").await?;
                return Ok(());
            }
        };

        println!("{:?}", request);

        keep_alive = request.wants_keep_alive();
        let connection_header = if keep_alive { "keep-alive" } else { "close" };

        let response = handle_request(&request);
        send_response(socket, &response, connection_header, "respuesta").await?;
    }

    Ok(())
}

async fn send_response(
    socket: &mut TcpStream,
    response: &Response,
    connection: &str,
    context: &str,
) -> Result<()> {
    socket
        .write_all(&response.to_http_bytes(connection))
        .await
        .with_context(|| format!("fallo al escribir {}", context))?;
    socket
        .flush()
        .await
        .with_context(|| format!("fallo al hacer flush {}", context))?;
    Ok(())
}

fn handle_request(request: &Request) -> Response {
    match (request.method(), request.path()) {
        (Method::Get, "/") => Response::ok(home_page()),
        (Method::Get, "/home") => Response::ok(home_page()),
        (Method::Get, _) => Response::not_found(),
        // Por ahora solo soportamos GET; cualquier otro método devuelve 405.
        _ => Response::method_not_allowed(),
    }
}

fn home_page() -> String {
    r#"<!DOCTYPE html>
<html>
<head><title>Mi Server</title></head>
<body><h1>¡Hola desde Rust!</h1></body>
</html>"#
    .to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_to_http_bytes() {
        let response = Response::ok("<h1>Hola</h1>");
        let bytes = response.to_http_bytes("close");
        let text = String::from_utf8(bytes).unwrap();

        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Type: text/html; charset=utf-8\r\n"));
        assert!(text.contains("Content-Length: 13\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.ends_with("<h1>Hola</h1>"));
    }

    #[test]
    fn test_response_to_http_bytes_keep_alive() {
        let response = Response::ok("<h1>Hola</h1>");
        let bytes = response.to_http_bytes("keep-alive");
        let text = String::from_utf8(bytes).unwrap();

        assert!(text.contains("Connection: keep-alive\r\n"));
    }

    #[test]
    fn test_not_found_response() {
        let response = Response::not_found();
        assert_eq!(response.status, 404);
        let bytes = response.to_http_bytes("close");
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 404 Not Found\r\n"));
    }

    #[test]
    fn test_method_not_allowed_response() {
        let response = Response::method_not_allowed();
        assert_eq!(response.status, 405);
        let bytes = response.to_http_bytes("close");
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"));
    }

    #[test]
    fn test_handle_request_get_root() {
        let raw = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 200);
        assert!(response.body.contains("¡Hola desde Rust!"));
    }

    #[test]
    fn test_handle_request_unknown_route() {
        let raw = "GET /desconocido HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 404);
    }

    #[test]
    fn test_handle_request_unsupported_method() {
        let raw = "POST / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 405);
    }
}
