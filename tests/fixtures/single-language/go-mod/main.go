package main

import (
    "net/http"
    "strconv"

    "github.com/gin-gonic/gin"
)

type User struct {
    ID    int    `json:"id"`
    Name  string `json:"name"`
    Email string `json:"email"`
}

var users = []User{
    {ID: 1, Name: "Alice", Email: "alice@example.com"},
    {ID: 2, Name: "Bob", Email: "bob@example.com"},
}

func main() {
    r := gin.Default()

    r.GET("/", func(c *gin.Context) {
        c.JSON(200, gin.H{
            "message":   "User API Server",
            "version":   "1.0.0",
            "endpoints": []string{"/users", "/users/:id", "/health"},
        })
    })

    r.GET("/health", func(c *gin.Context) {
        c.JSON(200, gin.H{"status": "healthy"})
    })

    r.GET("/users", func(c *gin.Context) {
        c.JSON(200, gin.H{"users": users})
    })

    r.GET("/users/:id", func(c *gin.Context) {
        id, _ := strconv.Atoi(c.Param("id"))
        for _, user := range users {
            if user.ID == id {
                c.JSON(200, gin.H{"user": user})
                return
            }
        }
        c.JSON(404, gin.H{"error": "User not found"})
    })

    r.POST("/users", func(c *gin.Context) {
        var newUser User
        if err := c.BindJSON(&newUser); err != nil {
            c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
            return
        }
        newUser.ID = len(users) + 1
        users = append(users, newUser)
        c.JSON(201, gin.H{"user": newUser})
    })

    r.Run()
}
