package com.example;

public class LibraryTest {
    public static void main(String[] args) {
        assert "Hello from library!".equals(Library.getMessage());
        System.out.println("Test passed!");
    }
}
