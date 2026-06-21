use std::borrow::Cow;
use std::path::Path;

use crate::response::Response;

const PUBLIC_DIR: &str = "public";

pub fn serve(request_path: &str) -> Option<Response> {
    let root = Path::new(PUBLIC_DIR).canonicalize().ok()?;
    let relative = request_path.trim_start_matches('/');

    if relative.is_empty() {
        return serve("index.html");
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
        Some("json") => Cow::Borrowed("application/json"),
        Some("png") => Cow::Borrowed("image/png"),
        Some("jpg") | Some("jpeg") => Cow::Borrowed("image/jpeg"),
        Some("gif") => Cow::Borrowed("image/gif"),
        Some("svg") => Cow::Borrowed("image/svg+xml"),
        Some("ico") => Cow::Borrowed("image/x-icon"),
        Some("txt") => Cow::Borrowed("text/plain; charset=utf-8"),
        _ => Cow::Borrowed("application/octet-stream"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_static_file_txt() {
        let response = serve("test.txt").unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/plain; charset=utf-8");
        assert_eq!(
            String::from_utf8_lossy(&response.body),
            "Hola desde un archivo estático"
        );
    }

    #[test]
    fn test_serve_static_file_html() {
        let response = serve("test.html").unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        assert!(String::from_utf8_lossy(&response.body).contains("Archivo estático HTML"));
    }

    #[test]
    fn test_serve_static_index_html() {
        let response = serve("index.html").unwrap();
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
        let response = serve("styles.css").unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/css; charset=utf-8");

        let body = String::from_utf8_lossy(&response.body);
        assert!(body.contains("--bg-color"));
        assert!(body.contains(".site-header"));
    }

    #[test]
    fn test_serve_static_app_js() {
        let response = serve("app.js").unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "application/javascript; charset=utf-8");

        let body = String::from_utf8_lossy(&response.body);
        assert!(body.contains("DOMContentLoaded"));
        assert!(body.contains("action-button"));
    }

    #[test]
    fn test_serve_static_blocks_traversal() {
        assert!(serve("../Cargo.toml").is_none());
    }

    #[test]
    fn test_serve_static_missing_file() {
        assert!(serve("no_existe.txt").is_none());
    }
}
