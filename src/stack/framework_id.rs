crate::define_id_enum! {
    /// Framework identifier with support for LLM-discovered frameworks
    FrameworkId {
        SpringBoot => "spring-boot" : "Spring Boot",
        Quarkus => "quarkus" : "Quarkus",
        Micronaut => "micronaut" : "Micronaut",
        Ktor => "ktor" : "Ktor",
        Express => "express" : "Express",
        NextJs => "nextjs" : "Next.js",
        NestJs => "nestjs" : "NestJS",
        Fastify => "fastify" : "Fastify",
        Django => "django" : "Django",
        Flask => "flask" : "Flask",
        FastApi => "fastapi" : "FastAPI",
        Rails => "rails" : "Rails",
        Sinatra => "sinatra" : "Sinatra",
        ActixWeb => "actix-web" : "Actix Web",
        Axum => "axum" : "Axum",
        Gin => "gin" : "Gin",
        Echo => "echo" : "Echo",
        AspNetCore => "aspnet-core" : "ASP.NET Core",
        Laravel => "laravel" : "Laravel",
        Symfony => "symfony" : "Symfony",
        Phoenix => "phoenix" : "Phoenix",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_framework_id_serialization() {
        assert_eq!(
            serde_json::to_string(&FrameworkId::SpringBoot).unwrap(),
            "\"spring-boot\""
        );
        assert_eq!(
            serde_json::to_string(&FrameworkId::NextJs).unwrap(),
            "\"nextjs\""
        );
    }

    #[test]
    fn test_framework_id_name() {
        assert_eq!(FrameworkId::SpringBoot.name(), "Spring Boot");
        assert_eq!(FrameworkId::NextJs.name(), "Next.js");
    }

    #[test]
    fn test_custom_framework_serialization() {
        let custom = FrameworkId::Custom("Fresh".to_string());
        assert_eq!(serde_json::to_string(&custom).unwrap(), "\"Fresh\"");
    }

    #[test]
    fn test_custom_framework_deserialization() {
        let deserialized: FrameworkId = serde_json::from_str("\"qwik\"").unwrap();
        assert_eq!(deserialized, FrameworkId::Custom("qwik".to_string()));
        assert_eq!(deserialized.name(), "qwik");
    }
}
