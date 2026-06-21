use crate::handlers;
use crate::request::{Method, Request};
use crate::response::Response;
use crate::static_files;

pub fn route(request: &Request) -> Response {
    match (request.method(), request.path()) {
        (Method::Get, "/") => handlers::home(),
        (Method::Get, "/home") => handlers::home(),
        (Method::Get, path) => static_files::serve(path).unwrap_or_else(Response::not_found),
        // Por ahora solo soportamos GET; cualquier otro método devuelve 405.
        _ => Response::method_not_allowed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_get_root() {
        let raw = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = route(&request);
        assert_eq!(response.status, 200);
        assert!(String::from_utf8_lossy(&response.body).contains("¡Hola desde Rust!"));
    }

    #[test]
    fn test_route_unknown_path() {
        let raw = "GET /desconocido HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = route(&request);
        assert_eq!(response.status, 404);
    }

    #[test]
    fn test_route_unsupported_method() {
        let raw = "POST / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw.as_bytes()).unwrap();
        let response = route(&request);
        assert_eq!(response.status, 405);
    }
}
