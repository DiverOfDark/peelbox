package com.example;

public class Library {
    public static String getMessage() {
        return "Hello from library!";
    }

    public static String toJson(String key, String value) {
        return "{\"" + key + "\":\"" + value + "\"}";
    }
}
