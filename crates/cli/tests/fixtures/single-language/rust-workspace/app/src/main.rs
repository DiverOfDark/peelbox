use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::Serialize;

#[derive(Serialize)]
struct GreetResponse {
    message: String,
}

#[derive(Serialize)]
struct MathResponse {
    operation: String,
    result: i32,
}

async fn index() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Workspace API Server",
        "version": "0.1.0"
    }))
}

async fn greet(name: web::Path<String>) -> impl Responder {
    let greeting = lib_a::greet(&name);
    HttpResponse::Ok().json(GreetResponse {
        message: greeting,
    })
}

async fn add(nums: web::Path<(i32, i32)>) -> impl Responder {
    let (a, b) = nums.into_inner();
    let result = lib_b::add(a, b);
    HttpResponse::Ok().json(MathResponse {
        operation: format!("{} + {}", a, b),
        result,
    })
}

async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy"
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting workspace server on http://127.0.0.1:8081");

    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(index))
            .route("/health", web::get().to(health))
            .route("/greet/{name}", web::get().to(greet))
            .route("/add/{a}/{b}", web::get().to(add))
    })
    .bind(("127.0.0.1", 8081))?
    .run()
    .await
}
