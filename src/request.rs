use std::collections::HashMap;

use anyhow::{anyhow, Result};

#[derive(Debug)]
pub struct Request {
    method: String,
    path: String,
    query: Option<String>,
    version: String,
    headers: HashMap<String, String>,
}

impl Request {
    pub fn new(request_payload: &[u8]) -> Result<Self> {
        let mut parsed_request = Request::parse_payload(request_payload)?;
        if parsed_request.is_empty() {
            return Err(anyhow!("petición vacía"));
        }
        let first = parsed_request.remove(0);
        let (method, path, query, version) = Request::extract_request_line(&first)?;
        let headers = Request::extract_headers(&parsed_request)?;
        Ok(Self {
            method: method.to_string(),
            path: path.to_string(),
            query: query.map(|q| q.to_string()),
            version: version.to_string(),
            headers,
        })
    }

    fn parse_payload(request_payload: &[u8]) -> Result<Vec<String>> {
        let parsed_request = String::from_utf8(request_payload.to_vec())?;
        Ok(parsed_request
            .split("\r\n")
            .map(|st| st.trim().to_string())
            .filter(|el| !el.is_empty())
            .collect())
    }

    fn extract_request_line(
        payload: &str,
    ) -> Result<(&str, &str, Option<&str>, &str)> {
        let mut parts = payload.split_whitespace();
        let method = parts.next().ok_or_else(|| anyhow!("falta method"))?;
        let target = parts.next().ok_or_else(|| anyhow!("falta path"))?;
        let version = parts.next().ok_or_else(|| anyhow!("falta version"))?;
        if parts.next().is_some() {
            return Err(anyhow!("línea de solicitud con demasiados elementos"));
        }

        let (path, query) = match target.split_once('?') {
            Some((path, query)) if !query.is_empty() => (path, Some(query)),
            Some((path, _)) => (path, None),
            None => (target, None),
        };

        Ok((method, path, query, version))
    }

    fn extract_headers(
        payload: &[String],
    ) -> Result<HashMap<String, String>> {
        let mut headers = HashMap::new();
        for line in payload {
            let (key, value) = line
                .split_once(':')
                .ok_or_else(|| anyhow!("header inválido: {}", line))?;
            headers.insert(key.trim().to_string(), value.trim().to_string());
        }
        Ok(headers)
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
            "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0 Safari/605.1.15",
            "Upgrade-Insecure-Requests: 1",
            "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "Sec-Fetch-Site: none",
            "Sec-Fetch-Mode: navigate",
            "Accept-Language: en-GB,en-US;q=0.9,en;q=0.8",
            "Priority: u=0, i",
            "Accept-Encoding: gzip, deflate",
            "Connection: keep-alive",
        ]
        .map(|st| st.to_string())
        .to_vec();
        let result = Request::extract_headers(&input).unwrap();
        assert_eq!(result["Sec-Fetch-Dest"], "document");
        assert_eq!(result["Host"], "localhost:8080");
    }

    #[test]
    fn test_extract_headers_invalid() {
        let input = ["Host localhost"].map(|st| st.to_string()).to_vec();
        assert!(Request::extract_headers(&input).is_err());
    }

    #[test]
    fn test_extract_request_line_with_query() {
        let input = "GET /home?query=hola&foo=bar HTTP/1.1";
        let (method, path, query, version) = Request::extract_request_line(input).unwrap();
        assert_eq!(method, "GET");
        assert_eq!(path, "/home");
        assert_eq!(query, Some("query=hola&foo=bar"));
        assert_eq!(version, "HTTP/1.1");
    }

    #[test]
    fn test_extract_request_line_without_query() {
        let input = "GET /home HTTP/1.1";
        let (method, path, query, version) = Request::extract_request_line(input).unwrap();
        assert_eq!(method, "GET");
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
    fn test_new_parses_full_request() {
        let raw_request = "GET /home?query=hola HTTP/1.1\r\n\
            Host: localhost:8080\r\n\
            Connection: keep-alive\r\n";
        let request = Request::new(raw_request.as_bytes()).unwrap();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/home");
        assert_eq!(request.query, Some("query=hola".to_string()));
        assert_eq!(request.version, "HTTP/1.1");
        assert_eq!(request.headers["Host"], "localhost:8080");
    }

    #[test]
    fn test_new_rejects_malformed_request() {
        let raw_request = "INVALID\r\n\r\n";
        assert!(Request::new(raw_request.as_bytes()).is_err());
    }

    #[test]
    fn test_parse_payload() {
        let raw_request = "GET /home?query=hola HTTP/1.1\r\n\
            Host: localhost:8080\r\n\
            Sec-Fetch-Dest: document\r\n\
            User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0 Safari/605.1.15\r\n\
            Upgrade-Insecure-Requests: 1\r\n\
            Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n\
            Sec-Fetch-Site: none\r\n\
            Sec-Fetch-Mode: navigate\r\n\
            Accept-Language: en-GB,en-US;q=0.9,en;q=0.8\r\n\
            Priority: u=0, i\r\n\
            Accept-Encoding: gzip, deflate\r\n\
            Connection: keep-alive\r\n";
        let expected_result = [
            "GET /home?query=hola HTTP/1.1",
            "Host: localhost:8080",
            "Sec-Fetch-Dest: document",
            "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0 Safari/605.1.15",
            "Upgrade-Insecure-Requests: 1",
            "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "Sec-Fetch-Site: none",
            "Sec-Fetch-Mode: navigate",
            "Accept-Language: en-GB,en-US;q=0.9,en;q=0.8",
            "Priority: u=0, i",
            "Accept-Encoding: gzip, deflate",
            "Connection: keep-alive"
        ]
        .map(|st| st.to_string())
        .to_vec();

        let bytes = raw_request.as_bytes().to_vec();
        let result = Request::parse_payload(&bytes).unwrap();

        assert_eq!(result, expected_result);
    }
}
