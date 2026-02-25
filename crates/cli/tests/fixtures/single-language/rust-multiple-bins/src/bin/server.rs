use std::io::{Read, Write};
use std::net::TcpListener;

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").expect("Failed to bind to port 8080");
    println!("Server listening on http://0.0.0.0:8080");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let request = String::from_utf8_lossy(&buffer);

                let (status, body) = if request.starts_with("GET /health") {
                    ("200 OK", r#"{"status":"healthy"}"#)
                } else {
                    ("200 OK", r#"{"message":"Hello from server"}"#)
                };

                let response = format!(
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    status,
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
            Err(e) => eprintln!("Connection error: {}", e),
        }
    }
}
