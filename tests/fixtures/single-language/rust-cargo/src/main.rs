use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Person {
    name: String,
    age: u32,
}

#[derive(Serialize)]
struct ApiResponse {
    status: String,
    data: Person,
}

async fn index() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "message": "Hello World API",
        "version": "0.1.0"
    }))
}

async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "uptime": "running"
    }))
}

async fn get_person() -> impl Responder {
    let person = Person {
        name: "Alice".to_string(),
        age: 30,
    };

    HttpResponse::Ok().json(ApiResponse {
        status: "success".to_string(),
        data: person,
    })
}

async fn create_person(person: web::Json<Person>) -> impl Responder {
    HttpResponse::Ok().json(ApiResponse {
        status: "created".to_string(),
        data: person.into_inner(),
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting server on http://0.0.0.0:8080");

    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(index))
            .route("/health", web::get().to(health))
            .route("/api/person", web::get().to(get_person))
            .route("/api/person", web::post().to(create_person))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_person_creation() {
        let person = Person {
            name: "Bob".to_string(),
            age: 25,
        };
        assert_eq!(person.name, "Bob");
        assert_eq!(person.age, 25);
    }
}
