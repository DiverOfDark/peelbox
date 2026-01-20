use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    if let Ok(_) = stream.read(&mut buffer) {
        let request = String::from_utf8_lossy(&buffer);
        let response = if request.starts_with("GET /health") {
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"healthy\"}"
        } else {
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"service\":\"Admin Service\",\"language\":\"Rust\"}"
        };
        let _ = stream.write(response.as_bytes());
        let _ = stream.flush();
    }
}

fn main() -> std::io::Result<()> {
    let port_str = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let port: u16 = port_str.parse().unwrap();
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
    println!("Starting admin service on 0.0.0.0:{}", port);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| {
                    handle_client(stream);
                });
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }
    Ok(())
}
