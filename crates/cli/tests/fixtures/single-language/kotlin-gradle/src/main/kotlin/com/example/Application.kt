package com.example

import com.sun.net.httpserver.HttpServer
import java.net.InetSocketAddress

fun main() {
    val port = System.getenv("PORT")?.toIntOrNull() ?: 8080
    val server = HttpServer.create(InetSocketAddress(port), 0)

    server.createContext("/") { exchange ->
        if (exchange.requestURI.path != "/") {
            val response = """{"error":"Not found"}"""
            exchange.responseHeaders.add("Content-Type", "application/json")
            exchange.sendResponseHeaders(404, response.toByteArray().size.toLong())
            exchange.responseBody.use { it.write(response.toByteArray()) }
            return@createContext
        }
        val response = """{"message":"Kotlin App","version":"1.0.0","endpoints":["/","/health"]}"""
        exchange.responseHeaders.add("Content-Type", "application/json")
        exchange.sendResponseHeaders(200, response.toByteArray().size.toLong())
        exchange.responseBody.use { it.write(response.toByteArray()) }
    }

    server.createContext("/health") { exchange ->
        val response = """{"status":"healthy"}"""
        exchange.responseHeaders.add("Content-Type", "application/json")
        exchange.sendResponseHeaders(200, response.toByteArray().size.toLong())
        exchange.responseBody.use { it.write(response.toByteArray()) }
    }

    server.executor = null
    server.start()
    println("Server started on port $port")
}
