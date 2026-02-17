use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    name: String,
    version: String,
}

fn main() {
    let config = Config {
        name: "rust-msrv-demo".to_string(),
        version: "0.1.0".to_string(),
    };

    let json = serde_json::to_string_pretty(&config).expect("Failed to serialize");
    println!("{}", json);
}
