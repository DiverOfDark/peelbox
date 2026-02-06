use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8082").expect("Failed to bind");
    println!("Admin service running on port 8082");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let reader = BufReader::new(&stream);
                let mut path = String::from("/");
                if let Some(Ok(request_line)) = reader.lines().next() {
                    let parts: Vec<&str> = request_line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        path = parts[1].to_string();
                    }
                }

                let body = match path.as_str() {
                    "/" => r#"{"service":"Admin Service","language":"Rust"}"#,
                    "/health" => r#"{"status":"healthy"}"#,
                    _ => r#"{"error":"not found"}"#,
                };

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
            Err(e) => eprintln!("Connection error: {}", e),
        }
    }
}
