use std::collections::HashMap;

#[derive(Debug)]
pub struct Request {
    method: String,
    path: String,
    version: String,
    headers: HashMap<String, String>,
}

impl Request {
    pub fn new(request_payload: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let mut parsed_request = Request::parse_payload(request_payload)?;
        let first = parsed_request.remove(0);
        let (method, path, version) = Request::extract_method_path_version(&first);
        let headers = Request::extract_headers(&parsed_request);
        Ok(Self {
            method: method.to_string(),
            path: path.to_string(),
            version: version.to_string(),
            headers,
        })
    }

    fn parse_payload(request_payload: &[u8]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let parsed_request = String::from_utf8(request_payload.to_vec())?;
        Ok(parsed_request
            .split("\r\n")
            .map(|st| st.trim().to_string())
            .filter(|el| !el.is_empty())
            .collect())
    }

    fn extract_method_path_version(payload: &str) -> (&str, &str, &str) {
        let mut result = payload.split_whitespace();
        let method = result.next().expect("falta method");
        let path = result.next().expect("falta path");
        let version = result.next().expect("falta version");
        (method, path, version)
    }

    fn extract_headers(payload: &Vec<String>) -> HashMap<String, String> {
        payload
            .iter()
            .map(|el| {
                let (key, value) = el.split_once(":").unwrap();
                (key.trim().to_string(), value.trim().to_string())
            })
            .collect()
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
        let result = Request::extract_headers(&input);
        assert_eq!(result["Sec-Fetch-Dest"], "document");
        assert_eq!(result["Host"], "localhost:8080");
    }

    #[test]
    fn test_extract_method_path_version() {
        let input = "GET /home?query=hola HTTP/1.1";
        let expected_method = "GET";
        let expected_path = "/home?query=hola";
        let expected_version = "HTTP/1.1";
        let (method, path, version) = Request::extract_method_path_version(input);
        assert_eq!(method, expected_method);
        assert_eq!(path, expected_path);
        assert_eq!(version, expected_version);
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
