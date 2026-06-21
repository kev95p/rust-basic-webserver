pub mod request;
pub mod response;

use std::borrow::Cow;
use std::path::Path;

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

        let response = handle_request(&request);
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

fn handle_request(request: &Request) -> Response {
    match (request.method(), request.path()) {
        (Method::Get, "/") => Response::ok(home_page()),
        (Method::Get, "/home") => Response::ok(home_page()),
        (Method::Get, path) => serve_static_file(path).unwrap_or_else(Response::not_found),
        // Por ahora solo soportamos GET; cualquier otro método devuelve 405.
        _ => Response::method_not_allowed(),
    }
}

fn serve_static_file(request_path: &str) -> Option<Response> {
    let root = Path::new("public").canonicalize().ok()?;
    let relative = request_path.trim_start_matches('/');

    if relative.is_empty() {
        return serve_static_file("index.html");
    }

    // Bloquear path traversal de forma simple y mediante canonicalización.
    if relative.contains("..") {
        return None;
    }

    let file_path = root.join(relative).canonicalize().ok()?;

    // Asegurar que el archivo resuelto siga dentro de public/.
    if !file_path.starts_with(&root) || !file_path.is_file() {
        return None;
    }

    let content = std::fs::read(&file_path).ok()?;
    let content_type = content_type_from_extension(&file_path);

    Some(Response::static_file(content_type, content))
}

fn content_type_from_extension(path: &Path) -> Cow<'static, str> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => Cow::Borrowed("text/html; charset=utf-8"),
        Some("css") => Cow::Borrowed("text/css; charset=utf-8"),
        Some("js") => Cow::Borrowed("application/javascript; charset=utf-8"),
        Some("json") => Cow::Borrowed("application/json; charset=utf-8"),
        Some("png") => Cow::Borrowed("image/png"),
        Some("jpg") | Some("jpeg") => Cow::Borrowed("image/jpeg"),
        Some("gif") => Cow::Borrowed("image/gif"),
        Some("svg") => Cow::Borrowed("image/svg+xml"),
        Some("ico") => Cow::Borrowed("image/x-icon"),
        Some("txt") => Cow::Borrowed("text/plain; charset=utf-8"),
        _ => Cow::Borrowed("application/octet-stream"),
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
        let bytes = response.to_http_bytes("close", None);
        let text = String::from_utf8(bytes).unwrap();

        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Type: text/html; charset=utf-8\r\n"));
        assert!(text.contains("Content-Length: 13\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(!text.contains("Content-Encoding:"));
        assert!(text.ends_with("<h1>Hola</h1>"));
    }

    #[test]
    fn test_response_to_http_bytes_keep_alive() {
        let response = Response::ok("<h1>Hola</h1>");
        let bytes = response.to_http_bytes("keep-alive", None);
        let text = String::from_utf8(bytes).unwrap();

        assert!(text.contains("Connection: keep-alive\r\n"));
    }

    #[test]
    fn test_response_to_http_bytes_gzip() {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let response = Response::ok("<h1>Hola</h1>");
        let bytes = response.to_http_bytes("close", Some("gzip"));
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Encoding: gzip\r\n"));
        assert!(text.contains("Connection: close\r\n"));

        // Separar headers del cuerpo comprimido.
        let separator = b"\r\n\r\n";
        let body_start = bytes
            .windows(separator.len())
            .position(|window| window == separator)
            .unwrap()
            + separator.len();
        let compressed = &bytes[body_start..];

        let mut decoder = GzDecoder::new(compressed);
        let mut decoded = String::new();
        decoder.read_to_string(&mut decoded).unwrap();
        assert_eq!(decoded, "<h1>Hola</h1>");
    }

    #[test]
    fn test_not_found_response() {
        let response = Response::not_found();
        assert_eq!(response.status, 404);
        let bytes = response.to_http_bytes("close", None);
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 404 Not Found\r\n"));
    }

    #[test]
    fn test_method_not_allowed_response() {
        let response = Response::method_not_allowed();
        assert_eq!(response.status, 405);
        let bytes = response.to_http_bytes("close", None);
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"));
    }

    #[test]
    fn test_handle_request_get_root() {
        let raw = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 200);
        assert!(String::from_utf8_lossy(&response.body).contains("¡Hola desde Rust!"));
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

    #[test]
    fn test_serve_static_file_txt() {
        let raw = "GET /test.txt HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/plain; charset=utf-8");
        assert_eq!(
            String::from_utf8_lossy(&response.body),
            "Hola desde un archivo estático"
        );
    }

    #[test]
    fn test_serve_static_file_html() {
        let raw = "GET /test.html HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        assert!(String::from_utf8_lossy(&response.body).contains("Archivo estático HTML"));
    }

    #[test]
    fn test_serve_static_file_blocks_traversal() {
        let raw = "GET /../Cargo.toml HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 404);
    }

    #[test]
    fn test_serve_static_file_missing_file() {
        let raw = "GET /no_existe.txt HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 404);
    }

    #[test]
    fn test_serve_static_index_html() {
        let raw = "GET /index.html HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");

        let body = String::from_utf8_lossy(&response.body);
        assert!(body.contains("<title>Servidor Rust</title>"));
        assert!(body.contains("href=\"/styles.css\""));
        assert!(body.contains("src=\"/app.js\""));
        assert!(body.contains("RustServer"));
    }

    #[test]
    fn test_serve_static_styles_css() {
        let raw = "GET /styles.css HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/css; charset=utf-8");

        let body = String::from_utf8_lossy(&response.body);
        assert!(body.contains("--bg-color"));
        assert!(body.contains(".site-header"));
    }

    #[test]
    fn test_serve_static_app_js() {
        let raw = "GET /app.js HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = handle_request(&request);
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "application/javascript; charset=utf-8");

        let body = String::from_utf8_lossy(&response.body);
        assert!(body.contains("DOMContentLoaded"));
        assert!(body.contains("action-button"));
    }
}
