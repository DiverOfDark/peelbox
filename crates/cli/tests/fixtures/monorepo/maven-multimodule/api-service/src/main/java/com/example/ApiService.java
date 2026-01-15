package com.example;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.web.bind.annotation.*;

import java.util.*;

@SpringBootApplication
@RestController
public class ApiService {
    public static void main(String[] args) {
        SpringApplication.run(ApiService.class, args);
    }

    @GetMapping("/")
    public Map<String, Object> index() {
        return Map.of(
                "service", "API Service",
                "library", Library.getMessage(),
                "endpoints", Arrays.asList("/", "/health", "/api/data")
        );
    }

    @GetMapping("/health")
    public Map<String, String> health() {
        return Map.of("status", "healthy", "service", "api");
    }

    @GetMapping("/api/data")
    public Map<String, Object> getData() {
        return Map.of(
                "data", Arrays.asList("item1", "item2", "item3"),
                "source", Library.getMessage()
        );
    }
}
