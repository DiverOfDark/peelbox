# Test Go Project

A sample Go project for testing aipack detection capabilities.

## Building

```bash
go build
```

Or for production:

```bash
go build -ldflags="-s -w" -o bin/app
```

## Testing

```bash
go test ./...
```

With coverage:

```bash
go test -cover ./...
```

## Running

```bash
go run main.go
```

## Formatting and Linting

```bash
# Format code
go fmt ./...

# Vet code
go vet ./...

# Run linter (if golangci-lint is installed)
golangci-lint run
```

## Dependencies

```bash
# Download dependencies
go mod download

# Tidy dependencies
go mod tidy

# Verify dependencies
go mod verify
```
