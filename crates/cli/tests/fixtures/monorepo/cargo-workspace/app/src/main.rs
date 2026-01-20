use actix_web::{get, App, HttpServer, Responder};

#[get("/")]
async fn index() -> impl Responder {
    format!("{} {}", lib_a::greet("Rust"), lib_b::add(2, 3))
}

#[get("/health")]
async fn health() -> impl Responder {
    "OK"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(index).service(health))
        .bind(("0.0.0.0", 8080))?
        .run()
        .await
}
