use std::borrow::Cow;

pub struct Response {
    pub status: u16,
    pub content_type: Cow<'static, str>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn ok(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            content_type: Cow::Borrowed("text/html; charset=utf-8"),
            body: body.into().into_bytes(),
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
            _ => "Unknown",
        }
    }

    pub fn to_http_bytes(&self, connection: &str, encoding: Option<&str>) -> Vec<u8> {
        let (body_bytes, content_encoding) = match encoding {
            Some("gzip") => {
                use flate2::write::GzEncoder;
                use flate2::Compression;
                use std::io::Write;

                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder
                    .write_all(&self.body)
                    .expect("fallo al escribir en el compresor gzip");
                let compressed = encoder
                    .finish()
                    .expect("fallo al finalizar la compresión gzip");
                (compressed, Some("gzip"))
            }
            _ => (self.body.clone(), None),
        };

        let mut headers = format!(
            "HTTP/1.1 {} {}\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Connection: {}\r\n",
            self.status,
            self.reason_phrase(),
            self.content_type,
            body_bytes.len(),
            connection,
        );

        if let Some(content_encoding) = content_encoding {
            headers.push_str(&format!("Content-Encoding: {}\r\n", content_encoding));
        }

        headers.push_str("\r\n");

        let mut response = headers.into_bytes();
        response.extend_from_slice(&body_bytes);
        response
    }
}
