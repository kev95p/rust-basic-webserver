use std::collections::HashMap;

use anyhow::{anyhow, Result};

const MAX_REQUEST_LINE_LEN: usize = 8 * 1024;
const MAX_HEADER_LEN: usize = 8 * 1024;
const MAX_HEADERS_COUNT: usize = 100;
const MAX_HEADERS_SIZE: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    Unsupported,
}

impl Method {
    fn parse(value: &str) -> Self {
        match value {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            "PATCH" => Self::Patch,
            "HEAD" => Self::Head,
            "OPTIONS" => Self::Options,
            _ => Self::Unsupported,
        }
    }
}

#[derive(Debug)]
pub struct Request {
    method: Method,
    path: String,
    #[allow(dead_code)]
    query: Option<String>,
    #[allow(dead_code)]
    version: String,
    headers: HashMap<String, String>,
}

impl Request {
    pub fn new(request_payload: &[u8]) -> Result<Self> {
        let (head, _body) = split_head_body(request_payload)?;
        let head = std::str::from_utf8(head)
            .map_err(|_| anyhow!("headers no son UTF-8 válido"))?;

        let mut lines = head.split("\r\n");
        let request_line = lines
            .next()
            .ok_or_else(|| anyhow!("petición vacía"))?
            .trim();

        if request_line.len() > MAX_REQUEST_LINE_LEN {
            return Err(anyhow!("request line demasiado larga"));
        }

        let (method, path, query, version) = Request::extract_request_line(request_line)?;
        let headers = Request::extract_headers(lines)?;

        Ok(Self {
            method,
            path: path.to_string(),
            query: query.map(std::string::ToString::to_string),
            version: version.to_string(),
            headers,
        })
    }

    pub fn method(&self) -> Method {
        self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    #[allow(dead_code)]
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    #[allow(dead_code)]
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn wants_keep_alive(&self) -> bool {
        match self.headers.get("connection") {
            Some(conn) if conn.eq_ignore_ascii_case("close") => false,
            Some(conn) if conn.eq_ignore_ascii_case("keep-alive") => true,
            // HTTP/1.1 mantiene la conexión abierta por defecto;
            // HTTP/1.0 la cierra por defecto.
            _ => self.version == "HTTP/1.1",
        }
    }

    pub fn accepts_encoding(&self, encoding: &str) -> bool {
        self.headers
            .get("accept-encoding")
            .is_some_and(|value| {
                value
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case(encoding))
            })
    }

    fn extract_request_line(
        payload: &str,
    ) -> Result<(Method, &str, Option<&str>, &str)> {
        let mut parts = payload.split_whitespace();
        let method_str = parts.next().ok_or_else(|| anyhow!("falta method"))?;
        let target = parts.next().ok_or_else(|| anyhow!("falta path"))?;
        let version = parts.next().ok_or_else(|| anyhow!("falta version"))?;
        if parts.next().is_some() {
            return Err(anyhow!("línea de solicitud con demasiados elementos"));
        }

        if version != "HTTP/1.0" && version != "HTTP/1.1" {
            return Err(anyhow!("versión HTTP no soportada: {version}"));
        }

        if target != "*" && !target.starts_with('/') {
            return Err(anyhow!("target inválido: {target}"));
        }

        let (path, query) = match target.split_once('?') {
            Some((path, query)) if !query.is_empty() => (path, Some(query)),
            Some((path, _)) => (path, None),
            None => (target, None),
        };

        Ok((Method::parse(method_str), path, query, version))
    }

    fn extract_headers<'a>(
        lines: impl Iterator<Item = &'a str>,
    ) -> Result<HashMap<String, String>> {
        let mut headers = HashMap::new();
        let mut total_size = 0usize;

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if headers.len() >= MAX_HEADERS_COUNT {
                return Err(anyhow!("demasiados headers"));
            }

            if line.len() > MAX_HEADER_LEN {
                return Err(anyhow!("header demasiado largo"));
            }

            total_size = total_size
                .checked_add(line.len())
                .ok_or_else(|| anyhow!("headers exceden tamaño máximo"))?;
            if total_size > MAX_HEADERS_SIZE {
                return Err(anyhow!("headers exceden tamaño máximo"));
            }

            let (key, value) = line
                .split_once(':')
                .ok_or_else(|| anyhow!("header inválido: {line}"))?;
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();

            headers
                .entry(key)
                .and_modify(|existing: &mut String| {
                    existing.push_str(", ");
                    existing.push_str(value);
                })
                .or_insert_with(|| value.to_string());
        }

        Ok(headers)
    }
}

fn split_head_body(payload: &[u8]) -> Result<(&[u8], &[u8])> {
    let separator = b"\r\n\r\n";
    match payload.windows(separator.len()).position(|w| w == separator) {
        Some(pos) => Ok((&payload[..pos], &payload[pos + separator.len()..])),
        None => Err(anyhow!("fin de headers no encontrado")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_headers() {
        let input = [
            "Host: localhost:8080",
            "Sec-Fetch-Dest: document",
            "Accept-Encoding: gzip, deflate",
            "Connection: keep-alive",
        ];
        let result = Request::extract_headers(input.into_iter()).unwrap();
        assert_eq!(result["sec-fetch-dest"], "document");
        assert_eq!(result["host"], "localhost:8080");
        assert_eq!(result["connection"], "keep-alive");
    }

    #[test]
    fn test_extract_headers_case_insensitive() {
        let input = ["connection: close"];
        let result = Request::extract_headers(input.into_iter()).unwrap();
        assert_eq!(result["connection"], "close");
    }

    #[test]
    fn test_extract_headers_repeated() {
        let input = ["Accept: text/html", "Accept: application/json"];
        let result = Request::extract_headers(input.into_iter()).unwrap();
        assert_eq!(result["accept"], "text/html, application/json");
    }

    #[test]
    fn test_extract_headers_invalid() {
        let input = ["Host localhost"];
        assert!(Request::extract_headers(input.into_iter()).is_err());
    }

    #[test]
    fn test_extract_request_line_with_query() {
        let input = "GET /home?query=hola&foo=bar HTTP/1.1";
        let (method, path, query, version) = Request::extract_request_line(input).unwrap();
        assert_eq!(method, Method::Get);
        assert_eq!(path, "/home");
        assert_eq!(query, Some("query=hola&foo=bar"));
        assert_eq!(version, "HTTP/1.1");
    }

    #[test]
    fn test_extract_request_line_without_query() {
        let input = "GET /home HTTP/1.1";
        let (method, path, query, version) = Request::extract_request_line(input).unwrap();
        assert_eq!(method, Method::Get);
        assert_eq!(path, "/home");
        assert_eq!(query, None);
        assert_eq!(version, "HTTP/1.1");
    }

    #[test]
    fn test_extract_request_line_empty_query() {
        let input = "GET /home? HTTP/1.1";
        let (_method, path, query, version) = Request::extract_request_line(input).unwrap();
        assert_eq!(path, "/home");
        assert_eq!(query, None);
        assert_eq!(version, "HTTP/1.1");
    }

    #[test]
    fn test_extract_request_line_missing_version() {
        let input = "GET /home";
        assert!(Request::extract_request_line(input).is_err());
    }

    #[test]
    fn test_extract_request_line_invalid_version() {
        let input = "GET /home HTTP/2.0";
        assert!(Request::extract_request_line(input).is_err());
    }

    #[test]
    fn test_extract_request_line_invalid_target() {
        let input = "GET http://example.com HTTP/1.1";
        assert!(Request::extract_request_line(input).is_err());
    }

    #[test]
    fn test_new_parses_full_request() {
        let raw_request = "GET /home?query=hola HTTP/1.1\r\n\
            Host: localhost:8080\r\n\
            Connection: keep-alive\r\n\r\n";
        let request = Request::new(raw_request.as_bytes()).unwrap();
        assert_eq!(request.method(), Method::Get);
        assert_eq!(request.path(), "/home");
        assert_eq!(request.query(), Some("query=hola"));
        assert_eq!(request.version(), "HTTP/1.1");
        assert_eq!(request.headers["host"], "localhost:8080");
    }

    #[test]
    fn test_new_rejects_malformed_request() {
        let raw_request = "INVALID\r\n\r\n";
        assert!(Request::new(raw_request.as_bytes()).is_err());
    }

    #[test]
    fn test_new_separates_body() {
        let raw_request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\nbody here";
        let request = Request::new(raw_request.as_bytes()).unwrap();
        assert_eq!(request.path(), "/");
        assert_eq!(request.headers["host"], "localhost");
    }

    #[test]
    fn test_wants_keep_alive_http11_default() {
        let raw_request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw_request.as_bytes()).unwrap();
        assert!(request.wants_keep_alive());
    }

    #[test]
    fn test_wants_keep_alive_with_close_header() {
        let raw_request = "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
        let request = Request::new(raw_request.as_bytes()).unwrap();
        assert!(!request.wants_keep_alive());
    }

    #[test]
    fn test_wants_keep_alive_lowercase_header() {
        let raw_request = "GET / HTTP/1.1\r\nHost: localhost\r\nconnection: close\r\n\r\n";
        let request = Request::new(raw_request.as_bytes()).unwrap();
        assert!(!request.wants_keep_alive());
    }

    #[test]
    fn test_wants_keep_alive_http10_default() {
        let raw_request = "GET / HTTP/1.0\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw_request.as_bytes()).unwrap();
        assert!(!request.wants_keep_alive());
    }

    #[test]
    fn test_wants_keep_alive_http10_with_keep_alive_header() {
        let raw_request =
            "GET / HTTP/1.0\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n";
        let request = Request::new(raw_request.as_bytes()).unwrap();
        assert!(request.wants_keep_alive());
    }

    #[test]
    fn test_accepts_encoding_gzip() {
        let raw_request = "GET / HTTP/1.1\r\nHost: localhost\r\nAccept-Encoding: gzip, deflate\r\n\r\n";
        let request = Request::new(raw_request.as_bytes()).unwrap();
        assert!(request.accepts_encoding("gzip"));
        assert!(request.accepts_encoding("deflate"));
        assert!(!request.accepts_encoding("br"));
    }

    #[test]
    fn test_accepts_encoding_missing() {
        let raw_request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::new(raw_request.as_bytes()).unwrap();
        assert!(!request.accepts_encoding("gzip"));
    }

    #[test]
    fn test_too_many_headers() {
        let lines: Vec<String> = (0..=MAX_HEADERS_COUNT)
            .map(|i| format!("X-Header-{i}: value"))
            .collect();
        assert!(Request::extract_headers(lines.iter().map(String::as_str)).is_err());
    }

    #[test]
    fn test_missing_header_terminator() {
        let raw_request = "GET / HTTP/1.1\r\nHost: localhost";
        assert!(Request::new(raw_request.as_bytes()).is_err());
    }
}
