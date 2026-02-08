package com.example;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;

@SpringBootApplication
@RestController
public class ApiService {
    public static void main(String[] args) {
        SpringApplication.run(ApiService.class, args);
    }

    @GetMapping("/")
    public String index() {
        return "{\"service\":\"API Service\",\"library\":\"" + Library.getMessage() + "\"}";
    }

    @GetMapping("/health")
    public String health() {
        return "{\"status\":\"healthy\",\"service\":\"api\"}";
    }

    @GetMapping("/api/data")
    public String data() {
        return "{\"data\":[\"item1\",\"item2\"],\"source\":\"" + Library.getMessage() + "\"}";
    }
}
