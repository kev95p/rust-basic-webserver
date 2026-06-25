use crate::handlers;
use crate::request::{Method, Request};
use crate::response::Response;
use crate::static_files;

/// Rutas definidas en el servidor.
const KNOWN_PATHS: &[&str] = &["/", "/home"];

pub async fn route(request: &Request) -> Response {
    match (request.method(), request.path()) {
        (Method::Unsupported, _) => Response::not_implemented(),
        (Method::Get | Method::Head, "/" | "/home") => handlers::home(request),
        (Method::Get | Method::Head, path) => static_files::serve(path)
            .await
            .unwrap_or_else(|e| {
                eprintln!("Error sirviendo archivo estático: {e}");
                Response::not_found()
            }),
        (_, path) if KNOWN_PATHS.contains(&path) => Response::method_not_allowed(),
        _ => Response::not_found(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_route_get_root() {
        let raw = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = route(&request).await;
        assert_eq!(response.status, 200);
        assert!(String::from_utf8_lossy(&response.body).contains("¡Hola desde Rust!"));
    }

    #[tokio::test]
    async fn test_route_head_root() {
        let raw = "HEAD / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = route(&request).await;
        assert_eq!(response.status, 200);
        assert!(!response.body.is_empty());
        // Nota: connection.rs descarta el body para HEAD antes de enviar.
    }

    #[tokio::test]
    async fn test_route_unknown_path() {
        let raw = "GET /desconocido HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = route(&request).await;
        assert_eq!(response.status, 404);
    }

    #[tokio::test]
    async fn test_route_post_unknown_path_returns_404() {
        let raw = "POST /desconocido HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = route(&request).await;
        assert_eq!(response.status, 404);
    }

    #[tokio::test]
    async fn test_route_post_root_returns_405() {
        let raw = "POST / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = route(&request).await;
        assert_eq!(response.status, 405);
    }

    #[tokio::test]
    async fn test_route_unsupported_method() {
        let raw = "TRACE / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = route(&request).await;
        assert_eq!(response.status, 501);
    }
}
