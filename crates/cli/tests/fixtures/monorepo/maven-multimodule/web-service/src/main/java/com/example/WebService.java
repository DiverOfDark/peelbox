package com.example;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;

@SpringBootApplication
@RestController
public class WebService {
    public static void main(String[] args) {
        SpringApplication.run(WebService.class, args);
    }

    @GetMapping("/")
    public String index() {
        return "{\"service\":\"Web Service\",\"library\":\"" + Library.getMessage() + "\"}";
    }

    @GetMapping("/health")
    public String health() {
        return "{\"status\":\"healthy\",\"service\":\"web\"}";
    }

    @GetMapping("/users")
    public String users() {
        return "{\"users\":[{\"name\":\"Alice\"},{\"name\":\"Bob\"}]}";
    }
}
