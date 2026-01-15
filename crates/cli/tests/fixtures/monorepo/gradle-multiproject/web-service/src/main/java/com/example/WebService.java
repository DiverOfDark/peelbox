package com.example;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.web.bind.annotation.*;

import java.util.*;

@SpringBootApplication
@RestController
public class WebService {
    private final List<User> users = new ArrayList<>(Arrays.asList(
            new User(1, "Alice", "alice@example.com"),
            new User(2, "Bob", "bob@example.com")
    ));

    public static void main(String[] args) {
        SpringApplication.run(WebService.class, args);
    }

    @GetMapping("/")
    public Map<String, Object> index() {
        return Map.of(
                "service", "Web Service",
                "library", Library.getMessage(),
                "endpoints", Arrays.asList("/", "/health", "/users")
        );
    }

    @GetMapping("/health")
    public Map<String, String> health() {
        return Map.of("status", "healthy", "service", "web");
    }

    @GetMapping("/users")
    public Map<String, List<User>> getUsers() {
        return Map.of("users", users);
    }

    @GetMapping("/users/{id}")
    public Map<String, Object> getUser(@PathVariable int id) {
        return users.stream()
                .filter(u -> u.getId() == id)
                .findFirst()
                .<Map<String, Object>>map(user -> Map.of("user", user))
                .orElse(Map.of("error", "User not found"));
    }

    @PostMapping("/users")
    public Map<String, User> createUser(@RequestBody User user) {
        user.setId(users.size() + 1);
        users.add(user);
        return Map.of("user", user);
    }

    static class User {
        private int id;
        private String name;
        private String email;

        public User() {}

        public User(int id, String name, String email) {
            this.id = id;
            this.name = name;
            this.email = email;
        }

        public int getId() { return id; }
        public void setId(int id) { this.id = id; }
        public String getName() { return name; }
        public void setName(String name) { this.name = name; }
        public String getEmail() { return email; }
        public void setEmail(String email) { this.email = email; }
    }
}
