package com.example

import com.sun.net.httpserver.{HttpServer, HttpHandler, HttpExchange}
import java.net.InetSocketAddress

object Main {
  def main(args: Array[String]): Unit = {
    val server = HttpServer.create(new InetSocketAddress(8080), 0)
    server.createContext("/health", new HttpHandler {
      def handle(exchange: HttpExchange): Unit = {
        val response = """{"status":"UP"}"""
        exchange.getResponseHeaders.set("Content-Type", "application/json")
        exchange.sendResponseHeaders(200, response.getBytes.length)
        val os = exchange.getResponseBody
        os.write(response.getBytes)
        os.close()
      }
    })
    server.createContext("/", new HttpHandler {
      def handle(exchange: HttpExchange): Unit = {
        val response = "Hello from Scala!"
        exchange.sendResponseHeaders(200, response.getBytes.length)
        val os = exchange.getResponseBody
        os.write(response.getBytes)
        os.close()
      }
    })
    server.setExecutor(null)
    server.start()
    println(s"Server started on port 8080")
  }
}
