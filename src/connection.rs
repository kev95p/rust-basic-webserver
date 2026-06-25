use anyhow::{anyhow, Context, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{timeout, Duration},
};

use crate::request::{Method, Request, RequestError};
use crate::response::Response;
use crate::router;

const MAX_REQUEST_SIZE: usize = 8 * 1024;
const MAX_BODY_SIZE: usize = 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const CHUNK_SIZE: usize = 1024;

pub async fn handle(mut socket: TcpStream) -> Result<()> {
    let peer = socket
        .peer_addr()
        .map_or_else(|_| "desconocido".to_string(), |addr| addr.to_string());
    let span = tracing::info_span!("connection", peer = %peer);
    let _enter = span.enter();

    let mut keep_alive = true;

    while keep_alive {
        let request_bytes = match read_request(&mut socket).await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                tracing::info!("cliente cerró la conexión");
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("error leyendo petición: {e}");
                let _ = send_response(
                    &mut socket,
                    Response::bad_request(),
                    "close",
                    None,
                    "respuesta 400",
                )
                .await;
                return Ok(());
            }
        };

        let request = match parse_request_with_body(&mut socket, request_bytes).await {
            Ok(req) => {
                tracing::info!(
                    method = ?req.method(),
                    path = req.path(),
                    "petición recibida"
                );
                req
            }
            Err(RequestError::ChunkedNotSupported) => {
                tracing::warn!("Transfer-Encoding chunked no soportado");
                send_response(
                    &mut socket,
                    Response::not_implemented(),
                    "close",
                    None,
                    "respuesta 501",
                )
                .await?;
                return Ok(());
            }
            Err(RequestError::IncompleteBody(missing)) => {
                tracing::warn!("body incompleto, faltan {missing} bytes");
                send_response(
                    &mut socket,
                    Response::bad_request(),
                    "close",
                    None,
                    "respuesta 400",
                )
                .await?;
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("petición malformada: {e}");
                send_response(
                    &mut socket,
                    Response::bad_request(),
                    "close",
                    None,
                    "respuesta 400",
                )
                .await?;
                return Ok(());
            }
        };

        keep_alive = request.wants_keep_alive();
        let connection_header = if keep_alive { "keep-alive" } else { "close" };
        let encoding = if request.accepts_encoding("gzip") {
            Some("gzip")
        } else {
            None
        };

        let mut response = router::route(&request).await;
        if request.method() == Method::Head {
            response = response.head();
        }

        send_response(
            &mut socket,
            response,
            connection_header,
            encoding,
            "respuesta",
        )
        .await?;
    }

    Ok(())
}

async fn parse_request_with_body(
    socket: &mut TcpStream,
    mut buffer: Vec<u8>,
) -> Result<Request, RequestError> {
    loop {
        match Request::new(&buffer) {
            Ok(request) => return Ok(request),
            Err(RequestError::IncompleteBody(missing)) => {
                if buffer.len() + missing > MAX_BODY_SIZE {
                    return Err(RequestError::Malformed(
                        "body excede tamaño máximo".to_string(),
                    ));
                }
                read_exact_bytes(socket, &mut buffer, missing).await?;
            }
            Err(e) => return Err(e),
        }
    }
}

async fn read_exact_bytes(
    socket: &mut TcpStream,
    buffer: &mut Vec<u8>,
    n: usize,
) -> Result<(), RequestError> {
    let mut temp = vec![0u8; n];
    let mut read = 0;

    while read < n {
        let count = timeout(READ_TIMEOUT, socket.read(&mut temp[read..]))
            .await
            .map_err(|_| RequestError::Malformed("timeout leyendo body".to_string()))?
            .map_err(|e| RequestError::Malformed(format!("error leyendo body: {e}")))?;

        if count == 0 {
            return Err(RequestError::IncompleteBody(n - read));
        }
        read += count;
    }

    buffer.extend_from_slice(&temp);
    Ok(())
}

async fn send_response(
    socket: &mut TcpStream,
    response: Response,
    connection: &str,
    encoding: Option<&str>,
    context: &str,
) -> Result<()> {
    let bytes = response.into_http_bytes(connection, encoding).unwrap_or_else(|e| {
        tracing::error!("fallo al serializar respuesta: {e}");
        Response::internal_server_error()
            .into_http_bytes("close", None)
            .unwrap_or_default()
    });

    socket
        .write_all(&bytes)
        .await
        .with_context(|| format!("fallo al escribir {context}"))?;
    socket
        .flush()
        .await
        .with_context(|| format!("fallo al hacer flush {context}"))?;
    Ok(())
}

async fn read_request(socket: &mut TcpStream) -> Result<Option<Vec<u8>>> {
    let mut buffer = Vec::with_capacity(CHUNK_SIZE);
    let mut temp = [0u8; CHUNK_SIZE];

    loop {
        let n = timeout(READ_TIMEOUT, socket.read(&mut temp))
            .await
            .with_context(|| "timeout leyendo del socket")??;

        if n == 0 {
            if buffer.is_empty() {
                // El cliente cerró la conexión de forma limpia.
                return Ok(None);
            }
            return Err(anyhow!(
                "Cliente cerró la conexión antes de enviar una petición completa"
            ));
        }

        if buffer.len() + n > MAX_REQUEST_SIZE {
            return Err(anyhow!("Petición demasiado grande"));
        }

        buffer.extend_from_slice(&temp[..n]);

        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(Some(buffer));
        }
    }
}


