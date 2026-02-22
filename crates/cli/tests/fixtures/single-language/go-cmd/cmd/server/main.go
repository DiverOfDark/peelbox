package main

import (
	"encoding/json"
	"fmt"
	"net/http"

	"example.com/go-cmd-app/internal"
)

func main() {
	http.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"status": "healthy"})
	})

	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{
			"service": "server",
			"version": internal.Version,
		})
	})

	fmt.Println("Starting server on :8080")
	http.ListenAndServe(":8080", nil)
}
