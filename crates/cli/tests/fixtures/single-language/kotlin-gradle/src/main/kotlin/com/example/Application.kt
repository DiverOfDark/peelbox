package com.example

import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication
import org.springframework.web.bind.annotation.*

data class User(var id: Int = 0, val name: String, val email: String)

@SpringBootApplication
@RestController
class Application {
    private val users = mutableListOf(
        User(1, "Alice", "alice@example.com"),
        User(2, "Bob", "bob@example.com")
    )

    @GetMapping("/")
    fun index() = mapOf(
        "message" to "User API Server",
        "version" to "1.0.0",
        "endpoints" to listOf("/users", "/users/{id}", "/health")
    )

    @GetMapping("/health")
    fun health() = mapOf("status" to "healthy")

    @GetMapping("/users")
    fun getUsers() = mapOf("users" to users)

    @GetMapping("/users/{id}")
    fun getUser(@PathVariable id: Int): Map<String, Any> {
        val user = users.find { it.id == id }
        return if (user != null) {
            mapOf("user" to user)
        } else {
            mapOf("error" to "User not found")
        }
    }

    @PostMapping("/users")
    fun createUser(@RequestBody user: User): Map<String, User> {
        user.id = users.size + 1
        users.add(user)
        return mapOf("user" to user)
    }
}

fun main(args: Array<String>) {
    runApplication<Application>(*args)
}
