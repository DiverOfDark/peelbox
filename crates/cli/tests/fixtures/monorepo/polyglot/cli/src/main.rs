use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    id: i32,
    key: String,
    value: String,
}

async fn index() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "service": "Admin Service",
        "language": "Rust",
        "endpoints": ["/", "/health", "/config"]
    }))
}

async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy"
    }))
}

async fn get_config() -> impl Responder {
    let configs = vec![
        Config { id: 1, key: "max_connections".to_string(), value: "100".to_string() },
        Config { id: 2, key: "timeout".to_string(), value: "30".to_string() },
    ];
    HttpResponse::Ok().json(serde_json::json!({ "configs": configs }))
}

async fn create_config(config: web::Json<Config>) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "config": config.into_inner()
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting admin service on http://127.0.0.1:8082");

    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(index))
            .route("/health", web::get().to(health))
            .route("/config", web::get().to(get_config))
            .route("/config", web::post().to(create_config))
    })
    .bind(("127.0.0.1", 8082))?
    .run()
    .await
}
