use std::borrow::Cow;
use std::io::{self, Write as _};
use std::time::SystemTime;

const GZIP_MIN_SIZE: usize = 20;

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub content_type: Cow<'static, str>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn ok(body: impl Into<Cow<'static, str>>) -> Self {
        let body = body.into();
        Self {
            status: 200,
            content_type: Cow::Borrowed("text/html; charset=utf-8"),
            body: body.into_owned().into_bytes(),
        }
    }

    #[allow(dead_code)]
    pub fn ok_bytes(content_type: impl Into<Cow<'static, str>>, body: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: content_type.into(),
            body,
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: 404,
            content_type: Cow::Borrowed("text/plain; charset=utf-8"),
            body: b"404 Not Found".to_vec(),
        }
    }

    pub fn method_not_allowed() -> Self {
        Self {
            status: 405,
            content_type: Cow::Borrowed("text/plain; charset=utf-8"),
            body: b"405 Method Not Allowed".to_vec(),
        }
    }

    pub fn bad_request() -> Self {
        Self {
            status: 400,
            content_type: Cow::Borrowed("text/plain; charset=utf-8"),
            body: b"400 Bad Request".to_vec(),
        }
    }

    pub fn not_implemented() -> Self {
        Self {
            status: 501,
            content_type: Cow::Borrowed("text/plain; charset=utf-8"),
            body: b"501 Not Implemented".to_vec(),
        }
    }

    pub fn internal_server_error() -> Self {
        Self {
            status: 500,
            content_type: Cow::Borrowed("text/plain; charset=utf-8"),
            body: b"500 Internal Server Error".to_vec(),
        }
    }

    pub fn static_file(content_type: impl Into<Cow<'static, str>>, body: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: content_type.into(),
            body,
        }
    }

    pub fn reason_phrase(&self) -> &'static str {
        match self.status {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            405 => "Method Not Allowed",
            500 => "Internal Server Error",
            501 => "Not Implemented",
            _ => "Unknown",
        }
    }

    /// Convierte la respuesta en bytes HTTP listos para enviar.
    ///
    /// # Errores
    /// Devuelve `std::io::Error` si la compresión gzip falla.
    ///
    /// # Seguridad
    /// Rechaza valores de `connection` o `content_type` que contengan
    /// caracteres de control de línea para evitar HTTP header injection.
    pub fn into_http_bytes(
        self,
        connection: &str,
        encoding: Option<&str>,
    ) -> io::Result<Vec<u8>> {
        validate_header_value(connection)?;
        validate_header_value(&self.content_type)?;

        let status = self.status;
        let reason = self.reason_phrase();
        let content_type = self.content_type;
        let should_gzip =
            encoding == Some("gzip") && self.body.len() > GZIP_MIN_SIZE;

        let (body_bytes, content_encoding) = if should_gzip {
            use flate2::write::GzEncoder;
            use flate2::Compression;

            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&self.body)?;
            let compressed = encoder.finish()?;
            (compressed, Some("gzip"))
        } else {
            (self.body, None)
        };

        let date = httpdate::fmt_http_date(SystemTime::now());

        let mut headers = format!(
            "HTTP/1.1 {status} {reason}\r\n\
             Date: {date}\r\n\
             Content-Type: {content_type}\r\n\
             Content-Length: {}\r\n\
             Connection: {connection}\r\n",
            body_bytes.len(),
        );

        if let Some(content_encoding) = content_encoding {
            headers.push_str("Content-Encoding: ");
            headers.push_str(content_encoding);
            headers.push_str("\r\n");
        }

        headers.push_str("\r\n");

        let mut response = headers.into_bytes();
        response.extend_from_slice(&body_bytes);
        Ok(response)
    }

    /// Devuelve una versión de esta respuesta sin cuerpo, útil para HEAD.
    pub fn head(mut self) -> Self {
        self.body.clear();
        self
    }
}

fn validate_header_value(value: &str) -> io::Result<()> {
    if value.bytes().any(|b| matches!(b, b'\r' | b'\n' | b'\0')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "valor de header HTTP inválido",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use std::io::Read;

    #[test]
    fn test_response_to_http_bytes() {
        let response = Response::ok("<h1>Hola</h1>");
        let bytes = response.into_http_bytes("close", None).unwrap();
        let text = String::from_utf8(bytes).unwrap();

        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Date: "));
        assert!(text.contains("Content-Type: text/html; charset=utf-8\r\n"));
        assert!(text.contains("Content-Length: 13\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(!text.contains("Content-Encoding:"));
        assert!(text.ends_with("<h1>Hola</h1>"));
    }

    #[test]
    fn test_response_to_http_bytes_keep_alive() {
        let response = Response::ok("<h1>Hola</h1>");
        let bytes = response.into_http_bytes("keep-alive", None).unwrap();
        let text = String::from_utf8(bytes).unwrap();

        assert!(text.contains("Connection: keep-alive\r\n"));
    }

    #[test]
    fn test_response_to_http_bytes_gzip() {
        let response = Response::ok("<h1>Hola</h1>".repeat(10));
        let bytes = response.into_http_bytes("close", Some("gzip")).unwrap();
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Encoding: gzip\r\n"));
        assert!(text.contains("Connection: close\r\n"));

        let separator = b"\r\n\r\n";
        let body_start = bytes
            .windows(separator.len())
            .position(|window| window == separator)
            .unwrap()
            + separator.len();
        let compressed = &bytes[body_start..];

        let mut decoder = GzDecoder::new(compressed);
        let mut output = String::new();
        decoder.read_to_string(&mut output).unwrap();
        assert_eq!(output, "<h1>Hola</h1>".repeat(10));
    }

    #[test]
    fn test_small_body_not_gzipped() {
        let response = Response::ok("hola");
        let bytes = response.into_http_bytes("close", Some("gzip")).unwrap();
        let text = String::from_utf8(bytes).unwrap();

        assert!(!text.contains("Content-Encoding:"));
        assert!(text.ends_with("hola"));
    }

    #[test]
    fn test_not_found_response() {
        let response = Response::not_found();
        assert_eq!(response.status, 404);
        let bytes = response.into_http_bytes("close", None).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 404 Not Found\r\n"));
    }

    #[test]
    fn test_method_not_allowed_response() {
        let response = Response::method_not_allowed();
        assert_eq!(response.status, 405);
        let bytes = response.into_http_bytes("close", None).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"));
    }

    #[test]
    fn test_internal_server_error_response() {
        let response = Response::internal_server_error();
        assert_eq!(response.status, 500);
        let bytes = response.into_http_bytes("close", None).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 500 Internal Server Error\r\n"));
    }

    #[test]
    fn test_not_implemented_response() {
        let response = Response::not_implemented();
        assert_eq!(response.status, 501);
        let bytes = response.into_http_bytes("close", None).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("HTTP/1.1 501 Not Implemented\r\n"));
    }

    #[test]
    fn test_header_injection_rejected() {
        let response = Response::ok("hola");
        let result = response.into_http_bytes("close\r\nX-Injected: yes", None);
        assert!(result.is_err());

        let response = Response::ok("hola");
        let result = response.into_http_bytes("close", None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_head_clears_body() {
        let response = Response::ok("<h1>Hola</h1>").head();
        assert!(response.body.is_empty());
    }
}
