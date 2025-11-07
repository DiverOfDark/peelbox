# aipack Usage Examples

This document provides real-world usage examples for aipack, from basic detection to advanced workflows and automation.

## Table of Contents

- [Basic Usage](#basic-usage)
- [Output Formats](#output-formats)
- [Backend Selection](#backend-selection)
- [Advanced Options](#advanced-options)
- [Error Handling](#error-handling)
- [Scripting and Automation](#scripting-and-automation)
- [Integration Examples](#integration-examples)
- [Troubleshooting Scenarios](#troubleshooting-scenarios)

## Basic Usage

### Detect Current Directory

The simplest usage - detect build system for the current directory:

```bash
cd /path/to/your/project
aipack detect
```

Example output:
```
Build System: cargo
Language: Rust
Build Command: cargo build --release
Test Command: cargo test
Deploy Command: cargo build --release
Confidence: 98%

Detected Files:
  - Cargo.toml
  - Cargo.lock
  - src/main.rs
  - src/lib.rs

Reasoning: Repository contains Cargo.toml with standard Rust project structure.
Binary crate with library and tests configured.

Processing Time: 2.3s
```

### Detect Specific Repository

Analyze a repository at a specific path:

```bash
aipack detect /path/to/nodejs/project
```

Example output:
```
Build System: npm
Language: TypeScript
Build Command: npm run build
Test Command: npm test
Deploy Command: npm run build && npm run deploy
Confidence: 95%

Detected Files:
  - package.json
  - tsconfig.json
  - src/index.ts

Reasoning: TypeScript project with npm scripts. Build script uses tsc compiler.
Test suite configured with Jest.
```

### Multiple Projects

Analyze several projects:

```bash
for dir in project1 project2 project3; do
    echo "Analyzing $dir..."
    aipack detect "$dir"
    echo "---"
done
```

## Output Formats

### JSON Output

Perfect for programmatic processing:

```bash
aipack detect --format json
```

Output:
```json
{
  "buildSystem": "cargo",
  "language": "Rust",
  "buildCommand": "cargo build --release",
  "testCommand": "cargo test",
  "deployCommand": "cargo build --release",
  "confidence": 0.98,
  "reasoning": "Repository contains Cargo.toml with standard Rust project structure",
  "detectedFiles": [
    "Cargo.toml",
    "Cargo.lock",
    "src/main.rs"
  ],
  "warnings": [],
  "processingTimeMs": 2340
}
```

### YAML Output

Human-readable structured format:

```bash
aipack detect --format yaml
```

Output:
```yaml
buildSystem: cargo
language: Rust
buildCommand: cargo build --release
testCommand: cargo test
deployCommand: cargo build --release
confidence: 0.98
reasoning: Repository contains Cargo.toml with standard Rust project structure
detectedFiles:
  - Cargo.toml
  - Cargo.lock
  - src/main.rs
warnings: []
processingTimeMs: 2340
```

### Parsing JSON with jq

Extract specific fields:

```bash
# Get just the build command
aipack detect --format json | jq -r '.buildCommand'
# Output: cargo build --release

# Get confidence percentage
aipack detect --format json | jq '.confidence * 100'
# Output: 98

# Check if confidence is high enough
if [ $(aipack detect --format json | jq '.confidence') > 0.9 ]; then
    echo "High confidence detection"
fi
```

## Backend Selection

### Using Ollama (Local)

Use local Ollama for privacy and offline operation:

```bash
# Make sure Ollama is running
ollama serve &

# Pull a model
ollama pull qwen:7b

# Use Ollama backend explicitly
aipack detect --backend ollama

# Or via environment variable
export AIPACK_BACKEND=ollama
aipack detect
```

### Using Mistral API

Use cloud API for faster inference:

```bash
# Set API key
export MISTRAL_API_KEY=your-api-key-here

# Use Mistral backend
aipack detect --backend mistral

# Or configure via environment
export AIPACK_BACKEND=mistral
export AIPACK_MISTRAL_MODEL=mistral-small
aipack detect
```

### Auto Backend Selection

Let aipack choose the best available backend:

```bash
# Tries Ollama first, falls back to Mistral if configured
aipack detect --backend auto

# Or just omit the flag (auto is default)
aipack detect
```

## Advanced Options

### Verbose Output

Get detailed information about the detection process:

```bash
aipack detect --verbose
```

Output includes:
- Repository scanning progress
- Files being analyzed
- Backend communication details
- Prompt sent to LLM
- Raw LLM response
- Parsing steps

### Custom Configuration

Override default settings:

```bash
# Use different Ollama endpoint
export AIPACK_OLLAMA_ENDPOINT=http://192.168.1.100:11434
aipack detect

# Use different model
export AIPACK_OLLAMA_MODEL=qwen:14b
aipack detect

# Combine multiple options
AIPACK_BACKEND=ollama \
AIPACK_OLLAMA_MODEL=qwen:14b \
RUST_LOG=aipack=debug \
aipack detect /path/to/repo --format json
```

### Timeout Configuration

Set custom timeouts for slow models or large repositories:

```bash
# Increase timeout to 2 minutes
export AIPACK_TIMEOUT=120
aipack detect /very/large/monorepo
```

## Error Handling

### Handling Backend Unavailability

Gracefully handle Ollama not running:

```bash
#!/bin/bash

# Try detection with error handling
if ! aipack detect --backend ollama 2>/tmp/aipack-error.log; then
    if grep -q "Ollama" /tmp/aipack-error.log; then
        echo "Ollama not running. Starting..."
        ollama serve &
        sleep 2
        aipack detect --backend ollama
    else
        echo "Detection failed: $(cat /tmp/aipack-error.log)"
        exit 1
    fi
fi
```

### Fallback Strategy

Try multiple backends:

```bash
#!/bin/bash

detect_with_fallback() {
    local repo="$1"

    # Try Ollama first
    if aipack detect "$repo" --backend ollama 2>/dev/null; then
        return 0
    fi

    # Fall back to Mistral
    if [ -n "$MISTRAL_API_KEY" ]; then
        aipack detect "$repo" --backend mistral
        return $?
    fi

    echo "No backends available" >&2
    return 1
}

detect_with_fallback /path/to/repo
```

### Validating Results

Check confidence before using results:

```bash
#!/bin/bash

result=$(aipack detect --format json)
confidence=$(echo "$result" | jq '.confidence')

if (( $(echo "$confidence < 0.8" | bc -l) )); then
    echo "Warning: Low confidence ($confidence). Manual review recommended."
    echo "$result" | jq '.reasoning'
    exit 1
fi

# Use results
build_cmd=$(echo "$result" | jq -r '.buildCommand')
echo "Running: $build_cmd"
eval "$build_cmd"
```

## Scripting and Automation

### Automatic Build Script

Detect and build automatically:

```bash
#!/bin/bash
# auto-build.sh - Detects and builds any repository

set -e

REPO_PATH="${1:-.}"
cd "$REPO_PATH"

echo "Detecting build system for $REPO_PATH..."
DETECTION=$(aipack detect --format json)

# Extract commands
BUILD_CMD=$(echo "$DETECTION" | jq -r '.buildCommand')
TEST_CMD=$(echo "$DETECTION" | jq -r '.testCommand')
CONFIDENCE=$(echo "$DETECTION" | jq -r '.confidence')

echo "Build System: $(echo "$DETECTION" | jq -r '.buildSystem')"
echo "Confidence: $(echo "$CONFIDENCE * 100" | bc)%"

# Verify confidence
if (( $(echo "$CONFIDENCE < 0.7" | bc -l) )); then
    echo "Error: Confidence too low for automatic build"
    exit 1
fi

# Run build
echo "Building with: $BUILD_CMD"
eval "$BUILD_CMD"

# Run tests if available
if [ "$TEST_CMD" != "null" ] && [ -n "$TEST_CMD" ]; then
    echo "Running tests: $TEST_CMD"
    eval "$TEST_CMD"
fi

echo "Build completed successfully!"
```

Usage:
```bash
chmod +x auto-build.sh
./auto-build.sh /path/to/any/project
```

### CI/CD Integration

GitHub Actions example:

```yaml
name: Auto Build

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install aipack
        run: cargo install aipack

      - name: Setup Ollama
        run: |
          curl -fsSL https://ollama.ai/install.sh | sh
          ollama serve &
          ollama pull qwen:7b

      - name: Detect and build
        run: |
          DETECTION=$(aipack detect --format json)
          BUILD_CMD=$(echo "$DETECTION" | jq -r '.buildCommand')
          TEST_CMD=$(echo "$DETECTION" | jq -r '.testCommand')

          echo "::notice::Build System: $(echo "$DETECTION" | jq -r '.buildSystem')"
          echo "::notice::Build Command: $BUILD_CMD"

          eval "$BUILD_CMD"
          eval "$TEST_CMD"
```

### Batch Analysis Report

Analyze multiple repositories and generate a report:

```bash
#!/bin/bash
# batch-analyze.sh - Analyze multiple repos and create CSV report

OUTPUT_FILE="analysis-report.csv"

echo "Repository,Build System,Language,Confidence,Build Command" > "$OUTPUT_FILE"

for repo in repos/*; do
    if [ -d "$repo" ]; then
        echo "Analyzing $(basename "$repo")..."

        result=$(aipack detect "$repo" --format json 2>/dev/null || echo "{}")

        if [ "$result" != "{}" ]; then
            name=$(basename "$repo")
            build_system=$(echo "$result" | jq -r '.buildSystem')
            language=$(echo "$result" | jq -r '.language')
            confidence=$(echo "$result" | jq -r '.confidence')
            build_cmd=$(echo "$result" | jq -r '.buildCommand')

            echo "$name,$build_system,$language,$confidence,\"$build_cmd\"" >> "$OUTPUT_FILE"
        fi
    fi
done

echo "Report generated: $OUTPUT_FILE"
cat "$OUTPUT_FILE" | column -t -s ','
```

### Pre-commit Hook

Validate build configuration before commits:

```bash
#!/bin/bash
# .git/hooks/pre-commit

echo "Validating build configuration..."

DETECTION=$(aipack detect --format json)
CONFIDENCE=$(echo "$DETECTION" | jq -r '.confidence')

if (( $(echo "$CONFIDENCE < 0.8" | bc -l) )); then
    echo "Warning: Build detection confidence is low ($CONFIDENCE)"
    echo "This might indicate misconfigured build files."
    read -p "Continue with commit? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi
```

## Integration Examples

### Docker Build Integration

Use aipack to auto-generate Dockerfile:

```bash
#!/bin/bash
# generate-dockerfile.sh

DETECTION=$(aipack detect --format json)
BUILD_SYSTEM=$(echo "$DETECTION" | jq -r '.buildSystem')
LANGUAGE=$(echo "$DETECTION" | jq -r '.language')
BUILD_CMD=$(echo "$DETECTION" | jq -r '.buildCommand')

case "$BUILD_SYSTEM" in
    cargo)
        cat > Dockerfile <<EOF
FROM rust:1.70 AS builder
WORKDIR /app
COPY . .
RUN $BUILD_CMD

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/* /usr/local/bin/
CMD ["/usr/local/bin/app"]
EOF
        ;;

    npm)
        cat > Dockerfile <<EOF
FROM node:18 AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN $BUILD_CMD

FROM node:18-slim
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY package*.json ./
RUN npm ci --production
CMD ["npm", "start"]
EOF
        ;;

    *)
        echo "Unsupported build system: $BUILD_SYSTEM"
        exit 1
        ;;
esac

echo "Generated Dockerfile for $BUILD_SYSTEM"
```

### Makefile Generation

Generate Makefile from detection:

```bash
#!/bin/bash
# generate-makefile.sh

DETECTION=$(aipack detect --format json)
BUILD_CMD=$(echo "$DETECTION" | jq -r '.buildCommand')
TEST_CMD=$(echo "$DETECTION" | jq -r '.testCommand')
DEPLOY_CMD=$(echo "$DETECTION" | jq -r '.deployCommand')

cat > Makefile <<EOF
.PHONY: build test deploy clean

build:
\t$BUILD_CMD

test:
\t$TEST_CMD

deploy:
\t$DEPLOY_CMD

clean:
\trm -rf target dist build node_modules

all: clean build test
EOF

echo "Generated Makefile"
```

### IDE Configuration

Generate VS Code tasks:

```bash
#!/bin/bash
# generate-vscode-tasks.sh

DETECTION=$(aipack detect --format json)
BUILD_CMD=$(echo "$DETECTION" | jq -r '.buildCommand')
TEST_CMD=$(echo "$DETECTION" | jq -r '.testCommand')

mkdir -p .vscode

cat > .vscode/tasks.json <<EOF
{
    "version": "2.0.0",
    "tasks": [
        {
            "label": "Build",
            "type": "shell",
            "command": "$BUILD_CMD",
            "group": {
                "kind": "build",
                "isDefault": true
            }
        },
        {
            "label": "Test",
            "type": "shell",
            "command": "$TEST_CMD",
            "group": {
                "kind": "test",
                "isDefault": true
            }
        }
    ]
}
EOF

echo "Generated .vscode/tasks.json"
```

## Troubleshooting Scenarios

### Low Confidence Detection

When confidence is low, investigate:

```bash
# Get detailed reasoning
DETECTION=$(aipack detect --format json)
CONFIDENCE=$(echo "$DETECTION" | jq -r '.confidence')
REASONING=$(echo "$DETECTION" | jq -r '.reasoning')
WARNINGS=$(echo "$DETECTION" | jq -r '.warnings[]')

if (( $(echo "$CONFIDENCE < 0.8" | bc -l) )); then
    echo "Low confidence detection: $CONFIDENCE"
    echo "Reasoning: $REASONING"
    echo "Warnings:"
    echo "$WARNINGS"

    echo -e "\nSuggestions:"
    echo "- Ensure build configuration files are present"
    echo "- Check if repository structure is standard"
    echo "- Try with a more powerful model (qwen:14b)"
fi
```

### Monorepo Handling

For monorepos, analyze subdirectories:

```bash
#!/bin/bash
# analyze-monorepo.sh

MONOREPO_ROOT="${1:-.}"

echo "Analyzing monorepo: $MONOREPO_ROOT"
echo "================================"

# Find potential project roots (directories with package.json, Cargo.toml, etc.)
find "$MONOREPO_ROOT" -type f \( -name "package.json" -o -name "Cargo.toml" -o -name "pom.xml" \) | while read -r config_file; do
    project_dir=$(dirname "$config_file")

    echo -e "\nProject: $project_dir"
    echo "---"

    DETECTION=$(aipack detect "$project_dir" --format json)
    echo "$DETECTION" | jq '{
        buildSystem,
        language,
        buildCommand,
        confidence
    }'
done
```

### Custom Output Processing

Create custom formatted output:

```bash
#!/bin/bash
# pretty-report.sh

DETECTION=$(aipack detect --format json)

# Extract fields
BUILD_SYSTEM=$(echo "$DETECTION" | jq -r '.buildSystem')
LANGUAGE=$(echo "$DETECTION" | jq -r '.language')
CONFIDENCE=$(echo "$DETECTION" | jq -r '.confidence * 100')
BUILD_CMD=$(echo "$DETECTION" | jq -r '.buildCommand')
TEST_CMD=$(echo "$DETECTION" | jq -r '.testCommand')

# Create pretty output
cat <<EOF
┌─────────────────────────────────────────────┐
│           Build System Detection            │
└─────────────────────────────────────────────┘

  Build System:  $BUILD_SYSTEM
  Language:      $LANGUAGE
  Confidence:    ${CONFIDENCE}%

  Commands:
    Build:  $BUILD_CMD
    Test:   $TEST_CMD

EOF
```

### Debugging Detection Issues

Enable full debugging:

```bash
# Maximum verbosity
RUST_LOG=aipack=trace aipack detect --verbose 2>&1 | tee debug.log

# View just the LLM prompt
grep -A 50 "Sending prompt" debug.log

# View just the response
grep -A 20 "Received response" debug.log

# Check for errors
grep -i "error\|warning" debug.log
```

## Performance Optimization

### Parallel Analysis

Analyze multiple repos in parallel:

```bash
#!/bin/bash
# parallel-analyze.sh

export -f analyze_repo

analyze_repo() {
    local repo=$1
    echo "Analyzing $repo..."
    aipack detect "$repo" --format json > "results/$(basename "$repo").json"
}

mkdir -p results

# Use GNU parallel
find repos/ -maxdepth 1 -type d | parallel -j 4 analyze_repo {}

# Or with xargs
find repos/ -maxdepth 1 -type d | xargs -P 4 -I {} bash -c 'analyze_repo "$@"' _ {}
```

### Caching Results

Cache detection results to avoid re-analysis:

```bash
#!/bin/bash
# cached-detect.sh

REPO_PATH="${1:-.}"
CACHE_DIR=".aipack-cache"
CACHE_FILE="$CACHE_DIR/$(echo "$REPO_PATH" | md5sum | cut -d' ' -f1).json"

mkdir -p "$CACHE_DIR"

# Check if cache exists and is recent (< 1 day old)
if [ -f "$CACHE_FILE" ] && [ $(find "$CACHE_FILE" -mtime -1 2>/dev/null) ]; then
    echo "Using cached result"
    cat "$CACHE_FILE"
else
    echo "Detecting (cache miss)..."
    aipack detect "$REPO_PATH" --format json | tee "$CACHE_FILE"
fi
```

## Real-World Use Cases

### Continuous Integration

```bash
#!/bin/bash
# ci-auto-build.sh - Universal CI build script

set -euo pipefail

echo "Starting AI-powered build..."

# Detect build system
DETECTION=$(aipack detect --format json)
CONFIDENCE=$(echo "$DETECTION" | jq -r '.confidence')

if (( $(echo "$CONFIDENCE < 0.75" | bc -l) )); then
    echo "::error::Build detection confidence too low: $CONFIDENCE"
    exit 1
fi

# Extract and run commands
BUILD_CMD=$(echo "$DETECTION" | jq -r '.buildCommand')
TEST_CMD=$(echo "$DETECTION" | jq -r '.testCommand')

echo "::group::Build"
eval "$BUILD_CMD"
echo "::endgroup::"

if [ "$TEST_CMD" != "null" ]; then
    echo "::group::Test"
    eval "$TEST_CMD"
    echo "::endgroup::"
fi

echo "Build successful!"
```

### Developer Onboarding

```bash
#!/bin/bash
# onboard.sh - Help new developers get started

clear
echo "Welcome to the project!"
echo "Analyzing repository..."
echo

DETECTION=$(aipack detect --format json)

cat <<EOF
This is a $(echo "$DETECTION" | jq -r '.language') project using $(echo "$DETECTION" | jq -r '.buildSystem').

To build the project, run:
  $(echo "$DETECTION" | jq -r '.buildCommand')

To run tests:
  $(echo "$DETECTION" | jq -r '.testCommand')

To deploy:
  $(echo "$DETECTION" | jq -r '.deployCommand')

Happy coding!
EOF
```

These examples demonstrate aipack's flexibility and power for automating build system detection and integration into various workflows.
