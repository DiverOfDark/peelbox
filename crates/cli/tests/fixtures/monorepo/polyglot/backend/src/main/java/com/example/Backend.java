package com.example;

import com.sun.net.httpserver.HttpServer;
import com.sun.net.httpserver.HttpExchange;
import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;

public class Backend {
    public static void main(String[] args) throws IOException {
        int port = 8080;
        HttpServer server = HttpServer.create(new InetSocketAddress("0.0.0.0", port), 0);

        server.createContext("/", exchange -> {
            String response = "{\"service\":\"Backend API\",\"language\":\"Java\",\"endpoints\":[\"/\",\"/health\"]}";
            sendJson(exchange, 200, response);
        });

        server.createContext("/health", exchange -> {
            String response = "{\"status\":\"healthy\"}";
            sendJson(exchange, 200, response);
        });

        server.setExecutor(null);
        server.start();
        System.out.println("Backend server running on port " + port);
    }

    private static void sendJson(HttpExchange exchange, int status, String body) throws IOException {
        byte[] bytes = body.getBytes(StandardCharsets.UTF_8);
        exchange.getResponseHeaders().set("Content-Type", "application/json");
        exchange.sendResponseHeaders(status, bytes.length);
        try (OutputStream os = exchange.getResponseBody()) {
            os.write(bytes);
        }
    }
}
