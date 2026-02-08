package com.example;

import static spark.Spark.*;

public class App {
    public static void main(String[] args) {
        port(8080);

        get("/", (req, res) -> {
            res.type("application/json");
            return "{\"message\": \"Hello from Gradle Java App\"}";
        });

        get("/health", (req, res) -> {
            res.type("application/json");
            return "{\"status\": \"healthy\"}";
        });
    }
}
