package com.example;

import com.sun.net.httpserver.HttpServer;
import com.sun.net.httpserver.HttpExchange;
import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;

public class ApiService {
    public static void main(String[] args) throws IOException {
        int port = 8080;
        String portEnv = System.getenv("PORT");
        if (portEnv != null && !portEnv.isEmpty()) {
            port = Integer.parseInt(portEnv);
        }

        HttpServer server = HttpServer.create(new InetSocketAddress(port), 0);

        server.createContext("/", exchange -> {
            if ("GET".equals(exchange.getRequestMethod())) {
                String response = "{\"service\":\"API Service\",\"library\":\"" + Library.getMessage() + "\"}";
                sendResponse(exchange, 200, response);
            } else {
                sendResponse(exchange, 405, "{\"error\":\"Method not allowed\"}");
            }
        });

        server.createContext("/health", exchange -> {
            if ("GET".equals(exchange.getRequestMethod())) {
                sendResponse(exchange, 200, "{\"status\":\"healthy\",\"service\":\"api\"}");
            } else {
                sendResponse(exchange, 405, "{\"error\":\"Method not allowed\"}");
            }
        });

        server.setExecutor(null);
        server.start();
        System.out.println("API Service started on port " + port);
    }

    private static void sendResponse(HttpExchange exchange, int statusCode, String body) throws IOException {
        byte[] responseBytes = body.getBytes("UTF-8");
        exchange.getResponseHeaders().set("Content-Type", "application/json");
        exchange.sendResponseHeaders(statusCode, responseBytes.length);
        try (OutputStream os = exchange.getResponseBody()) {
            os.write(responseBytes);
        }
    }
}
