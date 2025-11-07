# Contributing to aipack

Thank you for your interest in contributing to aipack! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Process](#development-process)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Documentation](#documentation)
- [Community](#community)

## Code of Conduct

### Our Pledge

We are committed to providing a welcoming and inspiring community for all. We pledge to make participation in our project and our community a harassment-free experience for everyone, regardless of age, body size, disability, ethnicity, gender identity and expression, level of experience, nationality, personal appearance, race, religion, or sexual identity and orientation.

### Our Standards

Examples of behavior that contributes to creating a positive environment:

- Using welcoming and inclusive language
- Being respectful of differing viewpoints and experiences
- Gracefully accepting constructive criticism
- Focusing on what is best for the community
- Showing empathy towards other community members

Examples of unacceptable behavior:

- Trolling, insulting/derogatory comments, and personal attacks
- Public or private harassment
- Publishing others' private information without explicit permission
- Other conduct which could reasonably be considered inappropriate in a professional setting

### Enforcement

Project maintainers are responsible for clarifying the standards of acceptable behavior and are expected to take appropriate and fair corrective action in response to any instances of unacceptable behavior.

## Getting Started

### Prerequisites

- **Rust 1.70+**: Install from [rustup.rs](https://rustup.rs/)
- **Git**: For version control
- **Ollama** (optional): For local testing

### Fork and Clone

1. Fork the repository on GitHub: https://github.com/diverofdark/aipack
2. Clone your fork locally:
   ```bash
   git clone https://github.com/your-username/aipack.git
   cd aipack
   ```

3. Add upstream remote:
   ```bash
   git remote add upstream https://github.com/diverofdark/aipack.git
   ```

4. Create a branch for your work:
   ```bash
   git checkout -b feature/your-feature-name
   ```

### Development Setup

1. Install dependencies:
   ```bash
   cargo fetch
   ```

2. Build the project:
   ```bash
   cargo build
   ```

3. Run tests:
   ```bash
   cargo test
   ```

4. Set up pre-commit hooks (optional):
   ```bash
   cat > .git/hooks/pre-commit <<'EOF'
   #!/bin/bash
   cargo fmt --check || exit 1
   cargo clippy -- -D warnings || exit 1
   cargo test || exit 1
   EOF
   chmod +x .git/hooks/pre-commit
   ```

## Development Process

### Finding Work

1. **Browse Issues**: Look for issues labeled `good first issue` or `help wanted`
2. **Discuss Features**: Open a discussion for new features before implementing
3. **Ask Questions**: Don't hesitate to ask for clarification in issues

### Making Changes

1. **Keep Changes Focused**: One feature or fix per PR
2. **Write Tests**: Add tests for new functionality
3. **Update Documentation**: Keep docs in sync with code changes
4. **Follow Conventions**: Adhere to project coding standards

### Commit Messages

Follow conventional commits format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types**:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

**Examples**:
```
feat(ollama): Add support for streaming responses

Implements streaming API for Ollama backend to provide
real-time feedback during long-running detections.

Closes #123
```

```
fix(parser): Handle malformed JSON responses gracefully

Previously, malformed JSON would crash the application.
Now we catch parse errors and provide helpful feedback.

Fixes #456
```

## Pull Request Process

### Before Submitting

Ensure your PR meets these requirements:

- [ ] Code compiles without errors
- [ ] All tests pass: `cargo test`
- [ ] Code is formatted: `cargo fmt`
- [ ] No clippy warnings: `cargo clippy`
- [ ] Documentation is updated
- [ ] CHANGELOG.md is updated (for notable changes)
- [ ] Commit messages follow conventions

### Submitting

1. **Push to your fork**:
   ```bash
   git push origin feature/your-feature-name
   ```

2. **Create Pull Request** on GitHub

3. **Fill out PR template** with:
   - Description of changes
   - Motivation and context
   - Related issues
   - Testing performed
   - Screenshots (if UI changes)

### PR Template

```markdown
## Description
Brief description of what this PR does.

## Motivation and Context
Why is this change needed? What problem does it solve?

Closes #(issue number)

## Type of Change
- [ ] Bug fix (non-breaking change fixing an issue)
- [ ] New feature (non-breaking change adding functionality)
- [ ] Breaking change (fix or feature causing existing functionality to change)
- [ ] Documentation update

## How Has This Been Tested?
Describe the tests you ran and how to reproduce them.

## Checklist
- [ ] Code follows project style guidelines
- [ ] Self-review of code completed
- [ ] Comments added for hard-to-understand areas
- [ ] Documentation updated
- [ ] No new warnings generated
- [ ] Tests added that prove fix/feature works
- [ ] All tests pass locally
- [ ] CHANGELOG.md updated (if applicable)
```

### Review Process

1. **Automated Checks**: CI must pass
2. **Code Review**: At least one maintainer approval required
3. **Address Feedback**: Respond to review comments
4. **Merge**: Maintainers will merge when ready

## Coding Standards

### Rust Style

Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/):

1. **Naming**:
   - Types: `PascalCase`
   - Functions/variables: `snake_case`
   - Constants: `SCREAMING_SNAKE_CASE`
   - Lifetimes: `'a`, `'b`, `'c`

2. **Formatting**:
   ```bash
   cargo fmt
   ```

3. **Linting**:
   ```bash
   cargo clippy
   ```

### Error Handling

- Use `Result` types for fallible operations
- Implement `thiserror::Error` for custom error types
- Provide helpful error messages
- Avoid `unwrap()` in library code
- Use `?` operator for error propagation

**Good**:
```rust
pub fn parse_config(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)
        .map_err(ConfigError::IoError)?;
    serde_json::from_str(&content)
        .map_err(|e| ConfigError::ParseError(e.to_string()))
}
```

**Bad**:
```rust
pub fn parse_config(path: &Path) -> Config {
    let content = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}
```

### Documentation

Every public item must have documentation:

```rust
/// Detects build system for a repository
///
/// Analyzes repository structure and uses LLM to determine
/// the appropriate build system and commands.
///
/// # Arguments
///
/// * `repo_path` - Path to repository root directory
///
/// # Returns
///
/// `DetectionResult` with build system information
///
/// # Errors
///
/// Returns `ServiceError` if:
/// - Repository path doesn't exist
/// - Backend is unavailable
/// - Detection fails
///
/// # Example
///
/// ```no_run
/// use aipack::detect;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let result = detect(PathBuf::from("/path/to/repo")).await?;
/// println!("Build: {}", result.build_command);
/// # Ok(())
/// # }
/// ```
pub async fn detect(repo_path: PathBuf) -> Result<DetectionResult, ServiceError> {
    // Implementation
}
```

### Async Code

- Use `async/await` for I/O operations
- Avoid blocking in async contexts
- Use `tokio::spawn` for concurrent tasks
- Document `Send` and `Sync` requirements

```rust
// Good - async I/O
async fn read_file(path: &Path) -> Result<String, io::Error> {
    tokio::fs::read_to_string(path).await
}

// Bad - blocking in async
async fn read_file_blocking(path: &Path) -> Result<String, io::Error> {
    std::fs::read_to_string(path)  // Blocks the executor!
}
```

## Testing Guidelines

### Test Coverage

Aim for high test coverage:
- Unit tests for individual functions
- Integration tests for workflows
- Documentation tests for examples

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = create_test_input();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected_value);
    }

    #[tokio::test]
    async fn test_async_function() {
        let result = async_function().await;
        assert!(result.is_ok());
    }

    #[test]
    #[should_panic(expected = "error message")]
    fn test_error_case() {
        function_that_should_panic();
    }
}
```

### Integration Tests

Create integration tests in `tests/` directory:

```rust
// tests/detection_integration.rs

use aipack::DetectionService;
use aipack::AipackConfig;
use tempfile::TempDir;

#[tokio::test]
async fn test_end_to_end_detection() {
    // Create test repository
    let temp_dir = TempDir::new().unwrap();
    let cargo_toml = temp_dir.path().join("Cargo.toml");
    std::fs::write(&cargo_toml, "[package]\nname = \"test\"").unwrap();

    // Run detection
    let config = AipackConfig::default();
    let service = DetectionService::new(&config).await.unwrap();
    let result = service.detect(temp_dir.path().to_path_buf()).await.unwrap();

    // Verify
    assert_eq!(result.build_system, "cargo");
}
```

### Running Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_ollama_detection

# Integration tests only
cargo test --test '*'

# With output
cargo test -- --nocapture

# With coverage
cargo tarpaulin --out Html
```

## Documentation

### Code Documentation

- Document all public APIs
- Include examples in doc comments
- Explain complex logic with inline comments
- Keep comments up-to-date with code

### Project Documentation

When adding features, update:
- `README.md` - User-facing documentation
- `docs/ARCHITECTURE.md` - Architectural changes
- `docs/DEVELOPMENT.md` - Development procedures
- `docs/EXAMPLES.md` - Usage examples
- `CHANGELOG.md` - Notable changes

### Building Documentation

```bash
# Build and view docs
cargo doc --no-deps --open

# Include private items
cargo doc --document-private-items
```

## Community

### Communication Channels

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Questions and ideas
- **Pull Requests**: Code contributions

### Getting Help

- Read the documentation in `docs/`
- Check existing issues and discussions
- Ask questions in GitHub Discussions
- Be patient and respectful

### Reporting Bugs

When reporting bugs, include:

1. **Environment**:
   - OS and version
   - Rust version
   - aipack version
   - Backend and model used

2. **Steps to Reproduce**:
   - Exact commands run
   - Repository structure (if public)
   - Expected vs actual behavior

3. **Logs**:
   ```bash
   RUST_LOG=aipack=debug aipack detect --verbose 2>&1 | tee debug.log
   ```

4. **Minimal Example**:
   - Simplest way to reproduce
   - Remove unrelated code

### Feature Requests

Before requesting a feature:

1. Check if already requested
2. Explain the use case
3. Describe the desired behavior
4. Discuss alternatives considered

### Reviewing Pull Requests

When reviewing PRs:

- Be constructive and respectful
- Explain reasoning for feedback
- Suggest specific improvements
- Acknowledge good work
- Test the changes if possible

## Recognition

Contributors will be:
- Listed in project README
- Mentioned in release notes
- Credited in commit history

Thank you for contributing to aipack!

## License

By contributing to aipack, you agree that your contributions will be licensed under the Apache License 2.0.
