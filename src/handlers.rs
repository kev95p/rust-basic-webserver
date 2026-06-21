use crate::response::Response;

pub fn home() -> Response {
    Response::ok(home_page())
}

fn home_page() -> String {
    r#"<!DOCTYPE html>
<html>
<head><title>Mi Server</title></head>
<body><h1>¡Hola desde Rust!</h1></body>
</html>"#
    .to_string()
}
