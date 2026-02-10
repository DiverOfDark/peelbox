use std::io::{Read, Write};
use std::net::TcpListener;

fn main() {
    let greeting = lib_a::greet("Rust");
    let sum = lib_b::add(2, 3);

    println!("Starting server on http://0.0.0.0:8080");
    let listener = TcpListener::bind(("0.0.0.0", 8080)).expect("Failed to bind");

    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);

            let body = format!(
                r#"{{"message":"{}","sum":{}}}"#,
                greeting, sum
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    }
}
