package com.example;

import com.sun.net.httpserver.HttpServer;
import com.sun.net.httpserver.HttpExchange;
import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;

public class ApiService {
    public static void main(String[] args) throws IOException {
        int port = 8080;
        HttpServer server = HttpServer.create(new InetSocketAddress(port), 0);

        server.createContext("/", exchange -> {
            if (!exchange.getRequestURI().getPath().equals("/")) {
                sendResponse(exchange, 404, "{\"error\":\"not found\"}");
                return;
            }
            String body = "{\"service\":\"API Service\",\"library\":\"" + Library.getMessage() + "\"}";
            sendResponse(exchange, 200, body);
        });

        server.createContext("/health", exchange -> {
            sendResponse(exchange, 200, Library.toJson("status", "healthy"));
        });

        server.createContext("/api/data", exchange -> {
            String body = "{\"data\":[\"item1\",\"item2\"],\"source\":\"" + Library.getMessage() + "\"}";
            sendResponse(exchange, 200, body);
        });

        server.setExecutor(null);
        server.start();
        System.out.println("API Service started on port " + port);
    }

    private static void sendResponse(HttpExchange exchange, int code, String body) throws IOException {
        byte[] bytes = body.getBytes("UTF-8");
        exchange.getResponseHeaders().set("Content-Type", "application/json");
        exchange.sendResponseHeaders(code, bytes.length);
        try (OutputStream os = exchange.getResponseBody()) {
            os.write(bytes);
        }
    }
}
