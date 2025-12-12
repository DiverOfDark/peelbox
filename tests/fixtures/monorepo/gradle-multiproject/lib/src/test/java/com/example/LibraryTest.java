package com.example;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.assertEquals;

public class LibraryTest {
    @Test
    public void testGetMessage() {
        assertEquals("Hello from library!", Library.getMessage());
    }
}
