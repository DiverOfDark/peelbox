package com.example;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.web.bind.annotation.*;

import javax.servlet.http.HttpServletRequest;
import java.util.*;

@SpringBootApplication
@RestController
public class Application {
    public static void main(String[] args) {
        SpringApplication.run(Application.class, args);
    }

    @GetMapping("/")
    public Map<String, String> index(HttpServletRequest request) {
        return Map.of("message", "Spring Boot 2 Application",
                       "path", request.getRequestURI());
    }

    @GetMapping("/users")
    public List<Map<String, String>> getUsers() {
        return List.of(
                Map.of("name", "Alice"),
                Map.of("name", "Bob")
        );
    }
}
