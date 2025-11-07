use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    name: String,
    version: String,
}

fn main() {
    println!("Hello from test Rust project!");

    let config = Config {
        name: "test-rust-project".to_string(),
        version: "0.1.0".to_string(),
    };

    println!("Config: {:?}", config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = Config {
            name: "test".to_string(),
            version: "1.0".to_string(),
        };

        assert_eq!(config.name, "test");
        assert_eq!(config.version, "1.0");
    }
}
