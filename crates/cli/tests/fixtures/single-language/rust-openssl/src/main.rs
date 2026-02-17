use openssl::ssl::{SslConnector, SslMethod};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    name: String,
}

fn main() {
    // Verify OpenSSL is linked and functional
    let connector = SslConnector::builder(SslMethod::tls());
    assert!(connector.is_ok(), "Failed to create SSL connector");

    let config: Config = serde_json::from_str(r#"{"name": "rust-openssl-demo"}"#).unwrap();

    println!("OpenSSL initialized successfully");
    println!("Application: {}", config.name);
    println!("TLS backend: native-tls (OpenSSL)");
}
