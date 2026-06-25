use crate::request::Request;
use crate::response::Response;

pub fn home(_request: &Request) -> Response {
    Response::ok(home_page())
}

const HOME_PAGE: &str = r"<!DOCTYPE html>
<html>
<head><title>Mi Server</title></head>
<body><h1>¡Hola desde Rust!</h1><a href='index.html'>index.html</a></body>
</html>";

fn home_page() -> &'static str {
    HOME_PAGE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_handler() {
        let request = Request::new("GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".as_bytes()).unwrap();
        let response = home(&request);
        assert_eq!(response.status, 200);
        let body = String::from_utf8_lossy(&response.body);
        assert!(body.contains("¡Hola desde Rust!"));
    }
}
