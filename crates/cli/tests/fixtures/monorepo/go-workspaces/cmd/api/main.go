package main

import (
	"fmt"
	"net/http"

	"github.com/example/go-workspaces/pkg/shared"
)

func main() {
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		greeting := shared.Greet("World")
		fmt.Fprintf(w, `{"message":"%s"}`, greeting)
	})

	fmt.Println("Starting server on :8080")
	http.ListenAndServe(":8080", nil)
}
