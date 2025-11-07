# aipack Troubleshooting Guide

This guide helps you diagnose and resolve common issues when using aipack.

## Table of Contents

- [Quick Diagnostics](#quick-diagnostics)
- [Installation Issues](#installation-issues)
- [Backend Problems](#backend-problems)
- [Detection Issues](#detection-issues)
- [Performance Problems](#performance-problems)
- [Configuration Errors](#configuration-errors)
- [Network Issues](#network-issues)
- [Common Error Messages](#common-error-messages)
- [Getting Help](#getting-help)

## Quick Diagnostics

### Health Check

Run a quick health check to identify issues:

```bash
# Check aipack version
aipack --version

# Check backend availability (future feature)
aipack health

# Test with verbose logging
RUST_LOG=aipack=debug aipack detect --verbose
```

### Environment Check

Verify your environment is correctly configured:

```bash
# Check Rust installation
rustc --version
cargo --version

# Check Ollama (if using local backend)
ollama --version
curl http://localhost:11434/api/tags

# Check environment variables
env | grep AIPACK
env | grep MISTRAL
env | grep RUST_LOG
```

### Minimal Test

Test with a simple known repository:

```bash
# Create test repository
mkdir -p /tmp/test-rust-project/src
echo '[package]\nname = "test"' > /tmp/test-rust-project/Cargo.toml
echo 'fn main() {}' > /tmp/test-rust-project/src/main.rs

# Test detection
aipack detect /tmp/test-rust-project
```

## Installation Issues

### Issue: `cargo install aipack` fails

**Symptoms**:
```
error: failed to compile aipack
```

**Solutions**:

1. **Update Rust**:
   ```bash
   rustup update stable
   rustc --version  # Should be 1.70+
   ```

2. **Clear cargo cache**:
   ```bash
   rm -rf ~/.cargo/registry/cache
   cargo clean
   cargo install aipack
   ```

3. **Install from source**:
   ```bash
   git clone https://github.com/diverofdark/aipack.git
   cd aipack
   cargo build --release
   sudo cp target/release/aipack /usr/local/bin/
   ```

### Issue: Binary not found after installation

**Symptoms**:
```bash
aipack: command not found
```

**Solutions**:

1. **Add cargo bin to PATH**:
   ```bash
   echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
   source ~/.bashrc
   ```

2. **Verify installation location**:
   ```bash
   ls ~/.cargo/bin/aipack
   # If exists, add to PATH or use full path
   ```

3. **Install to system path**:
   ```bash
   cargo build --release
   sudo install -m 755 target/release/aipack /usr/local/bin/
   ```

### Issue: Permission denied during installation

**Symptoms**:
```
error: failed to create directory `/usr/local/bin`
```

**Solution**:
```bash
# Install to user directory
cargo install --root ~/.local aipack
export PATH="$HOME/.local/bin:$PATH"

# Or use sudo for system-wide install
sudo cargo install --root /usr/local aipack
```

## Backend Problems

### Issue: Ollama connection refused

**Symptoms**:
```
Error: Backend error
Help: Cannot connect to Ollama
Details: Connection refused at http://localhost:11434
```

**Solutions**:

1. **Start Ollama**:
   ```bash
   # Check if Ollama is running
   ps aux | grep ollama

   # Start Ollama
   ollama serve

   # Or as background service
   nohup ollama serve > /tmp/ollama.log 2>&1 &
   ```

2. **Verify Ollama is accessible**:
   ```bash
   curl http://localhost:11434/api/tags
   # Should return JSON with available models
   ```

3. **Check firewall**:
   ```bash
   # Allow port 11434
   sudo ufw allow 11434
   ```

4. **Use different endpoint**:
   ```bash
   export AIPACK_OLLAMA_ENDPOINT=http://127.0.0.1:11434
   aipack detect
   ```

### Issue: Ollama model not found

**Symptoms**:
```
Error: Model 'qwen:7b' not found
```

**Solutions**:

1. **Pull the model**:
   ```bash
   ollama pull qwen:7b

   # Or use a different model
   ollama pull qwen:14b
   export AIPACK_OLLAMA_MODEL=qwen:14b
   ```

2. **List available models**:
   ```bash
   ollama list
   ```

3. **Use an existing model**:
   ```bash
   # Find available model
   ollama list

   # Use it
   export AIPACK_OLLAMA_MODEL=llama2:7b
   aipack detect
   ```

### Issue: Mistral API key invalid

**Symptoms**:
```
Error: Authentication failed
Details: Invalid API key
```

**Solutions**:

1. **Verify API key**:
   ```bash
   echo $MISTRAL_API_KEY
   # Should show your key (mk-...)
   ```

2. **Set API key correctly**:
   ```bash
   export MISTRAL_API_KEY=your-actual-key-here
   # Add to ~/.bashrc for persistence
   ```

3. **Get new API key**:
   - Go to https://console.mistral.ai/
   - Create or regenerate API key
   - Update environment variable

4. **Check key permissions**:
   - Ensure key has API access enabled
   - Verify account is active
   - Check for quota limits

### Issue: Backend timeout

**Symptoms**:
```
Error: Request timeout after 30 seconds
Help: The LLM request took too long
```

**Solutions**:

1. **Increase timeout**:
   ```bash
   export AIPACK_TIMEOUT=120  # 2 minutes
   aipack detect
   ```

2. **Use faster model**:
   ```bash
   # Switch to smaller/faster model
   export AIPACK_OLLAMA_MODEL=qwen:7b  # Instead of qwen:14b
   aipack detect
   ```

3. **Check system resources**:
   ```bash
   # Ensure Ollama has enough resources
   top
   # Look for ollama process, check CPU/memory
   ```

4. **Reduce repository size** (if very large):
   ```bash
   # Analyze subdirectory instead
   aipack detect /path/to/repo/src
   ```

## Detection Issues

### Issue: Low confidence scores

**Symptoms**:
```
Confidence: 45%
Warning: Low confidence detection
```

**Causes & Solutions**:

1. **Missing key files**:
   ```bash
   # Check repository has build configuration
   ls -la Cargo.toml package.json pom.xml build.gradle

   # Add missing files
   # For Rust: Add Cargo.toml
   # For Node: Add package.json
   ```

2. **Unusual project structure**:
   - Ensure project follows standard conventions
   - Add README explaining build process
   - Include standard configuration files

3. **Try different model**:
   ```bash
   # Use larger model for better accuracy
   export AIPACK_OLLAMA_MODEL=qwen:14b
   aipack detect
   ```

4. **Multiple build systems**:
   - Project might have multiple build options
   - Check warnings for more context
   - Manually verify suggested commands

### Issue: Incorrect build system detected

**Symptoms**:
```
Build System: npm
# But project actually uses yarn
```

**Solutions**:

1. **Check for conflicting files**:
   ```bash
   # Multiple lock files can confuse detection
   ls -la package-lock.json yarn.lock pnpm-lock.yaml

   # Remove unused lock files
   rm package-lock.json  # If using yarn
   ```

2. **Add clear indicators**:
   ```bash
   # Add scripts to package.json
   # Add explicit build tool version files
   echo "nodeLinker: node-modules" > .yarnrc.yml
   ```

3. **Use verbose mode to understand**:
   ```bash
   RUST_LOG=aipack=debug aipack detect --verbose 2>&1 | less
   # Look for files detected and reasoning
   ```

4. **Verify detection results**:
   ```bash
   # Test suggested command
   DETECTION=$(aipack detect --format json)
   BUILD_CMD=$(echo "$DETECTION" | jq -r '.buildCommand')

   # Test it
   $BUILD_CMD
   ```

### Issue: Empty or null results

**Symptoms**:
```json
{
  "buildSystem": "",
  "buildCommand": null,
  ...
}
```

**Solutions**:

1. **Check repository is valid**:
   ```bash
   # Ensure directory is not empty
   ls -la /path/to/repo

   # Check it's a project directory
   find . -name "*.toml" -o -name "*.json" -o -name "*.xml"
   ```

2. **Enable verbose logging**:
   ```bash
   RUST_LOG=aipack=debug aipack detect --verbose
   # Look for parsing errors
   ```

3. **Check LLM response**:
   ```bash
   # Enable trace logging
   RUST_LOG=aipack=trace aipack detect 2>&1 | grep -A 20 "LLM response"
   ```

4. **Try different backend**:
   ```bash
   # If using Ollama, try Mistral
   export MISTRAL_API_KEY=your-key
   aipack detect --backend mistral
   ```

## Performance Problems

### Issue: Detection is very slow

**Symptoms**:
- Takes more than 30 seconds
- System becomes unresponsive

**Solutions**:

1. **Use faster model**:
   ```bash
   # Switch to smaller model
   export AIPACK_OLLAMA_MODEL=qwen:7b
   ```

2. **Check system resources**:
   ```bash
   # Monitor during detection
   htop

   # Check disk I/O
   iostat -x 1

   # Check if swapping
   vmstat 1
   ```

3. **Reduce context size** (future feature):
   ```bash
   # For now, analyze subdirectory
   aipack detect /path/to/repo/src
   ```

4. **Use SSD for Ollama models**:
   ```bash
   # Move Ollama models to SSD
   # Default: ~/.ollama/models
   ```

### Issue: High memory usage

**Symptoms**:
```
Out of memory error
```

**Solutions**:

1. **Use smaller model**:
   ```bash
   ollama pull qwen:7b  # ~4GB RAM
   # Instead of qwen:14b (~8GB RAM)
   ```

2. **Close other applications**:
   ```bash
   # Free up memory before running
   ```

3. **Increase swap space**:
   ```bash
   # Linux: Increase swap
   sudo fallocate -l 4G /swapfile
   sudo chmod 600 /swapfile
   sudo mkswap /swapfile
   sudo swapon /swapfile
   ```

### Issue: Repository too large

**Symptoms**:
```
Error: Repository too large
Help: Repository exceeded file limit
```

**Solutions**:

1. **Analyze subdirectory**:
   ```bash
   # Instead of entire monorepo
   aipack detect /path/to/repo/packages/api
   ```

2. **Clean build artifacts**:
   ```bash
   # Remove generated files
   rm -rf target/ node_modules/ dist/ build/

   # Then analyze
   aipack detect
   ```

3. **Use .gitignore patterns** (future feature):
   - Currently aipack respects .gitignore
   - Ensure large directories are ignored

## Configuration Errors

### Issue: Environment variables not recognized

**Symptoms**:
```
Using default configuration instead of env vars
```

**Solutions**:

1. **Export variables properly**:
   ```bash
   # Incorrect (no export)
   AIPACK_BACKEND=ollama

   # Correct
   export AIPACK_BACKEND=ollama

   # Verify
   env | grep AIPACK
   ```

2. **Use .env file**:
   ```bash
   # Create .env file
   cat > .env <<EOF
   AIPACK_BACKEND=ollama
   AIPACK_OLLAMA_MODEL=qwen:7b
   RUST_LOG=aipack=info
   EOF

   # Load it
   source .env
   aipack detect
   ```

3. **Check variable names**:
   ```bash
   # Correct names
   AIPACK_BACKEND=ollama
   AIPACK_OLLAMA_ENDPOINT=http://localhost:11434
   AIPACK_OLLAMA_MODEL=qwen:7b

   # Not: OLLAMA_ENDPOINT (missing AIPACK_ prefix)
   ```

### Issue: Invalid configuration values

**Symptoms**:
```
Error: Configuration error
Details: Invalid backend: xyz
```

**Solutions**:

1. **Use valid backend names**:
   ```bash
   # Valid: ollama, mistral, auto
   export AIPACK_BACKEND=ollama

   # Invalid: qwen, local, ai
   ```

2. **Check model name format**:
   ```bash
   # For Ollama, use format: model:tag
   export AIPACK_OLLAMA_MODEL=qwen:7b
   # Not: qwen-7b or qwen7b
   ```

3. **Verify endpoint format**:
   ```bash
   # Correct
   export AIPACK_OLLAMA_ENDPOINT=http://localhost:11434

   # Incorrect (missing http://)
   export AIPACK_OLLAMA_ENDPOINT=localhost:11434
   ```

## Network Issues

### Issue: Cannot connect to Ollama on remote server

**Symptoms**:
```
Connection refused at http://remote-server:11434
```

**Solutions**:

1. **Check Ollama is listening on all interfaces**:
   ```bash
   # On remote server
   OLLAMA_HOST=0.0.0.0:11434 ollama serve
   ```

2. **Verify firewall rules**:
   ```bash
   # On remote server
   sudo ufw allow 11434

   # Check listening
   netstat -tlnp | grep 11434
   ```

3. **Test connectivity**:
   ```bash
   # From client
   curl http://remote-server:11434/api/tags

   # If this fails, it's a network issue
   ```

4. **Use SSH tunnel**:
   ```bash
   # Create tunnel
   ssh -L 11434:localhost:11434 user@remote-server

   # Use localhost
   export AIPACK_OLLAMA_ENDPOINT=http://localhost:11434
   aipack detect
   ```

### Issue: Mistral API network errors

**Symptoms**:
```
Error: Network error
Details: Connection timeout
```

**Solutions**:

1. **Check internet connectivity**:
   ```bash
   ping api.mistral.ai
   curl -I https://api.mistral.ai
   ```

2. **Check proxy settings**:
   ```bash
   # If behind corporate proxy
   export HTTP_PROXY=http://proxy:8080
   export HTTPS_PROXY=http://proxy:8080
   ```

3. **Verify DNS resolution**:
   ```bash
   nslookup api.mistral.ai
   dig api.mistral.ai
   ```

4. **Use different DNS**:
   ```bash
   # Use Google DNS
   echo "nameserver 8.8.8.8" | sudo tee /etc/resolv.conf
   ```

## Common Error Messages

### "Repository path not found"

**Full error**:
```
Error: Repository path not found: /path/to/repo
Help: The specified path does not exist
```

**Solution**:
```bash
# Check path exists
ls -la /path/to/repo

# Use absolute path
aipack detect "$(realpath /path/to/repo)"

# Check current directory
aipack detect .
```

### "Repository path is not a directory"

**Full error**:
```
Error: Repository path is not a directory: /path/to/file
```

**Solution**:
```bash
# Don't specify a file, specify directory
aipack detect /path/to/repo  # Not /path/to/repo/Cargo.toml
```

### "Backend not yet implemented"

**Full error**:
```
Error: Backend not yet implemented
Details: Claude and OpenAI backends are not yet implemented
```

**Solution**:
```bash
# Use Ollama or Mistral
export AIPACK_BACKEND=ollama
aipack detect
```

### "Failed to parse LLM response"

**Full error**:
```
Error: Failed to parse LLM response
Details: JSON parsing error
```

**Solutions**:

1. **Retry the operation**:
   ```bash
   # LLM might have returned malformed JSON
   aipack detect
   ```

2. **Try different model**:
   ```bash
   # Some models are better at JSON output
   export AIPACK_OLLAMA_MODEL=qwen:14b
   aipack detect
   ```

3. **Check raw response**:
   ```bash
   RUST_LOG=aipack=debug aipack detect --verbose 2>&1 | grep -A 50 "LLM response"
   # Check if response is valid JSON
   ```

4. **Report issue**:
   - If persistent, file bug report
   - Include repository structure
   - Include LLM model and version

## Getting Help

### Collect Diagnostic Information

Before requesting help, collect:

```bash
#!/bin/bash
# collect-diagnostics.sh

echo "=== aipack Diagnostics ===" > diagnostics.txt
echo >> diagnostics.txt

echo "Version:" >> diagnostics.txt
aipack --version >> diagnostics.txt 2>&1
echo >> diagnostics.txt

echo "Environment:" >> diagnostics.txt
env | grep AIPACK >> diagnostics.txt
env | grep RUST_LOG >> diagnostics.txt
env | grep MISTRAL >> diagnostics.txt
echo >> diagnostics.txt

echo "Ollama Status:" >> diagnostics.txt
curl -s http://localhost:11434/api/tags >> diagnostics.txt 2>&1
echo >> diagnostics.txt

echo "Test Detection:" >> diagnostics.txt
RUST_LOG=aipack=debug aipack detect . --verbose >> diagnostics.txt 2>&1

echo "Diagnostics saved to diagnostics.txt"
```

### Enable Debug Logging

```bash
# Maximum logging
RUST_LOG=aipack=trace aipack detect --verbose 2>&1 | tee debug.log

# Share debug.log when requesting help
```

### Report Issues

When filing an issue, include:

1. **Environment**:
   - OS and version
   - Rust version (`rustc --version`)
   - aipack version (`aipack --version`)
   - Backend (Ollama/Mistral) and version

2. **Reproduction**:
   - Exact command run
   - Repository structure (if public, link to repo)
   - Expected vs actual behavior

3. **Logs**:
   - Debug logs (`RUST_LOG=aipack=debug`)
   - Error messages
   - Backend responses (if relevant)

4. **Configuration**:
   - Environment variables set
   - Custom configuration
   - Backend model used

### Community Support

- **GitHub Issues**: https://github.com/diverofdark/aipack/issues
- **Discussions**: https://github.com/diverofdark/aipack/discussions
- **Documentation**: Check docs/ directory
- **Examples**: See docs/EXAMPLES.md

### FAQ

**Q: Can aipack work offline?**
A: Yes, when using Ollama backend with models already downloaded.

**Q: How much RAM do I need?**
A: Minimum 8GB for qwen:7b, 16GB recommended for qwen:14b.

**Q: Can I use my own LLM?**
A: Yes, implement the `LLMBackend` trait (see docs/DEVELOPMENT.md).

**Q: Does aipack send my code to external servers?**
A: Only if using Mistral API backend. Ollama runs locally.

**Q: Can I cache detection results?**
A: Not built-in yet, but you can implement caching (see docs/EXAMPLES.md).

**Q: Why is detection slow?**
A: LLM inference can take 2-10 seconds. Use faster models or local Ollama for better performance.
