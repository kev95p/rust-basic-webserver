pub mod request;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use crate::request::Request;

const MAX_REQUEST_SIZE: usize = 8 * 1024;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tcp_listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Servidor escuchando en 0.0.0.0:8080");

    loop {
        let (mut socket, _) = tcp_listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(&mut socket).await {
                eprintln!("Error en conexión: {}", e);
            }
        });
    }
}

async fn handle_connection(socket: &mut TcpStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let request_bytes = read_request(socket).await?;
    let request = match Request::new(&request_bytes) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("Petición malformada: {}", e);
            let response = "HTTP/1.1 400 Bad Request\r\n\
                Content-Length: 0\r\n\
                Connection: close\r\n\
                \r\n";
            socket.write_all(response.as_bytes()).await?;
            return Ok(());
        }
    };
    println!("{:?}", request);

    let html = r#"<!DOCTYPE html>
<html>
<head><title>Mi Server</title></head>
<body><h1>¡Hola desde Rust!</h1></body>
</html>"#;

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        html.as_bytes().len(),
        html
    );

    socket.write_all(response.as_bytes()).await?;
    Ok(())
}

async fn read_request(socket: &mut TcpStream) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 1024];

    loop {
        let n = socket.read(&mut temp).await?;
        if n == 0 {
            return Err("Cliente cerró la conexión antes de enviar una petición completa".into());
        }

        if buffer.len() + n > MAX_REQUEST_SIZE {
            return Err("Petición demasiado grande".into());
        }

        buffer.extend_from_slice(&temp[..n]);

        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(buffer);
        }
    }
}
