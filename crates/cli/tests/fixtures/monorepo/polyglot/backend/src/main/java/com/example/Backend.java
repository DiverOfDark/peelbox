package com.example;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.web.bind.annotation.*;

import java.util.*;

@SpringBootApplication
@RestController
public class Backend {
    private final List<Product> products = new ArrayList<>(Arrays.asList(
            new Product(1, "Product A", 29.99),
            new Product(2, "Product B", 49.99)
    ));

    public static void main(String[] args) {
        SpringApplication.run(Backend.class, args);
    }

    @GetMapping("/")
    public Map<String, Object> index() {
        return Map.of(
                "service", "Backend API",
                "language", "Java",
                "endpoints", Arrays.asList("/", "/health", "/products")
        );
    }

    @GetMapping("/health")
    public Map<String, String> health() {
        return Map.of("status", "healthy");
    }

    @GetMapping("/products")
    public Map<String, List<Product>> getProducts() {
        return Map.of("products", products);
    }

    @GetMapping("/products/{id}")
    public Map<String, Object> getProduct(@PathVariable int id) {
        return products.stream()
                .filter(p -> p.getId() == id)
                .findFirst()
                .<Map<String, Object>>map(product -> Map.of("product", product))
                .orElse(Map.of("error", "Product not found"));
    }

    @PostMapping("/products")
    public Map<String, Product> createProduct(@RequestBody Product product) {
        product.setId(products.size() + 1);
        products.add(product);
        return Map.of("product", product);
    }

    static class Product {
        private int id;
        private String name;
        private double price;

        public Product() {}

        public Product(int id, String name, double price) {
            this.id = id;
            this.name = name;
            this.price = price;
        }

        public int getId() { return id; }
        public void setId(int id) { this.id = id; }
        public String getName() { return name; }
        public void setName(String name) { this.name = name; }
        public double getPrice() { return price; }
        public void setPrice(double price) { this.price = price; }
    }
}
