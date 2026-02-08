package com.example

import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication
import org.springframework.web.bind.annotation.GetMapping
import org.springframework.web.bind.annotation.RestController

@SpringBootApplication
@RestController
open class Application {
    @GetMapping("/")
    fun index(): String {
        return """{"message":"Kotlin App","version":"1.0.0","endpoints":["/","/health"]}"""
    }

    @GetMapping("/health")
    fun health(): String {
        return """{"status":"healthy"}"""
    }
}

fun main(args: Array<String>) {
    runApplication<Application>(*args)
}
