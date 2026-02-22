package main

import (
	"fmt"

	"example.com/go-cmd-app/internal"
)

func main() {
	fmt.Printf("Worker %s starting\n", internal.Version)
	fmt.Println("Processing tasks...")
	fmt.Println("Worker finished")
}
