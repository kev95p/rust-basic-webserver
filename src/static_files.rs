use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use percent_encoding::percent_decode_str;
use thiserror::Error;
use tokio::fs;

use crate::response::Response;

const PUBLIC_DIR: &str = "public";
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

static ROOT: LazyLock<PathBuf> =
    LazyLock::new(|| Path::new(PUBLIC_DIR).canonicalize().unwrap_or_else(|_| PathBuf::from(PUBLIC_DIR)));

#[derive(Debug, Error)]
pub enum StaticFileError {
    #[error("path traversal detectado")]
    BadPath,
    #[error("archivo no encontrado")]
    NotFound,
    #[error("archivo demasiado grande")]
    TooLarge,
    #[error("error de lectura: {0}")]
    Io(#[from] std::io::Error),
}

pub async fn serve(request_path: &str) -> Result<Response, StaticFileError> {
    let decoded = percent_decode_str(request_path).decode_utf8_lossy();
    let relative = decoded.trim_start_matches('/');

    if relative.is_empty() {
        serve_path("index.html").await
    } else {
        serve_path(relative).await
    }
}

async fn serve_path(relative: &str) -> Result<Response, StaticFileError> {
    let root = ROOT.clone();
    let file_path = match root.join(relative).canonicalize() {
        Ok(path) => path,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(StaticFileError::NotFound);
        }
        Err(e) => return Err(StaticFileError::Io(e)),
    };

    // Asegurar que el archivo resuelto siga dentro de public/.
    if !file_path.starts_with(&root) {
        return Err(StaticFileError::BadPath);
    }

    if !file_path.is_file() {
        return Err(StaticFileError::NotFound);
    }

    let metadata = fs::metadata(&file_path).await?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(StaticFileError::TooLarge);
    }

    let content = fs::read(&file_path).await?;
    let content_type = content_type_from_extension(&file_path);

    Ok(Response::static_file(content_type, content))
}

fn content_type_from_extension(path: &Path) -> Cow<'static, str> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html" | "htm") => Cow::Borrowed("text/html; charset=utf-8"),
        Some("css") => Cow::Borrowed("text/css; charset=utf-8"),
        Some("js") => Cow::Borrowed("application/javascript; charset=utf-8"),
        Some("json") => Cow::Borrowed("application/json"),
        Some("png") => Cow::Borrowed("image/png"),
        Some("jpg" | "jpeg") => Cow::Borrowed("image/jpeg"),
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

    #[tokio::test]
    async fn test_serve_static_file_txt() {
        let response = serve("test.txt").await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/plain; charset=utf-8");
        assert_eq!(
            String::from_utf8_lossy(&response.body),
            "Hola desde un archivo estático"
        );
    }

    #[tokio::test]
    async fn test_serve_static_file_html() {
        let response = serve("test.html").await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        assert!(String::from_utf8_lossy(&response.body).contains("Archivo estático HTML"));
    }

    #[tokio::test]
    async fn test_serve_static_index_html() {
        let response = serve("index.html").await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");

        let body = String::from_utf8_lossy(&response.body);
        assert!(body.contains("<title>Servidor Rust</title>"));
        assert!(body.contains("href=\"/styles.css\""));
        assert!(body.contains("src=\"/app.js\""));
        assert!(body.contains("RustServer"));
    }

    #[tokio::test]
    async fn test_serve_static_styles_css() {
        let response = serve("styles.css").await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/css; charset=utf-8");

        let body = String::from_utf8_lossy(&response.body);
        assert!(body.contains("--bg-color"));
        assert!(body.contains(".site-header"));
    }

    #[tokio::test]
    async fn test_serve_static_app_js() {
        let response = serve("app.js").await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "application/javascript; charset=utf-8");

        let body = String::from_utf8_lossy(&response.body);
        assert!(body.contains("DOMContentLoaded"));
        assert!(body.contains("action-button"));
    }

    #[tokio::test]
    async fn test_serve_static_blocks_traversal() {
        assert!(matches!(
            serve("../Cargo.toml").await.unwrap_err(),
            StaticFileError::BadPath | StaticFileError::NotFound
        ));
    }

    #[tokio::test]
    async fn test_serve_static_missing_file() {
        assert!(matches!(
            serve("no_existe.txt").await.unwrap_err(),
            StaticFileError::NotFound
        ));
    }

    #[tokio::test]
    async fn test_serve_static_decodes_percent_encoding() {
        let response = serve("test%2Etxt").await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/plain; charset=utf-8");
    }
}
