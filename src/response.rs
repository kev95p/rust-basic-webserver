pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

impl Response {
    pub fn ok(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: body.into(),
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: 404,
            content_type: "text/plain; charset=utf-8",
            body: "404 Not Found".to_string(),
        }
    }

    pub fn method_not_allowed() -> Self {
        Self {
            status: 405,
            content_type: "text/plain; charset=utf-8",
            body: "405 Method Not Allowed".to_string(),
        }
    }

    pub fn bad_request() -> Self {
        Self {
            status: 400,
            content_type: "text/plain; charset=utf-8",
            body: "400 Bad Request".to_string(),
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

    pub fn to_http_bytes(&self, connection: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 {} {}\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Connection: {}\r\n\
             \r\n\
             {}",
            self.status,
            self.reason_phrase(),
            self.content_type,
            self.body.as_bytes().len(),
            connection,
            self.body
        )
        .into_bytes()
    }
}
