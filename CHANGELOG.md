# Changelog - aipack

All notable changes to aipack will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **LM Studio Backend**: Support for local LLM inference via LM Studio's OpenAI-compatible API
  - New `LMStudioClient` implementing `LLMBackend` trait
  - OpenAI-compatible message format support
  - Health check and timeout handling
  - Automatic backend selection: Ollama → LM Studio → Mistral
  - Configurable endpoint (default: `http://localhost:8000`)
  - Environment variable: `AIPACK_LM_STUDIO_ENDPOINT`
  - Comprehensive unit and integration tests

### Planned for Phase 2
- Claude API backend integration
- OpenAI GPT backend integration
- Result caching system
- Custom model support
- Improved prompt engineering
- Multi-language monorepo support

### Planned for Phase 3
- HTTP/REST API service
- Web UI dashboard
- Batch processing capabilities
- Docker integration templates
- Kubernetes manifests
- Performance optimization and profiling

## [0.1.0] - 2025-11-07 (MVP Release)

### Added

#### Core Features
- **LLM Backend System**:
  - Abstract `LLMBackend` trait for pluggable AI providers
  - Ollama client implementation with health checks and timeout support
  - Support for multiple Qwen models (qwen2.5-coder:7b, qwen:14b, qwen:32b)
  - Auto backend selection (tries Ollama first, falls back to configured alternatives)

- **Repository Analysis**:
  - Comprehensive repository analyzer with configurable depth and limits
  - File tree generation with intelligent truncation
  - Key configuration file detection (Cargo.toml, package.json, pom.xml, etc.)
  - Git metadata extraction (branch, remote, commit status)
  - Respects .gitignore patterns
  - Configurable ignore patterns for build artifacts

- **Detection Service**:
  - High-level orchestration service for detection workflow
  - Path validation and error handling
  - Backend health verification
  - Performance metric tracking
  - Result enrichment with metadata

- **CLI Interface**:
  - `detect` command for build system detection
  - Multiple output formats: JSON, YAML, human-readable
  - Verbose mode for debugging
  - Environment variable configuration
  - Helpful error messages with troubleshooting hints

- **Confidence Scoring**:
  - LLM-generated confidence scores (0.0 - 1.0)
  - Confidence-based recommendations
  - Warning generation for low-confidence detections

- **Result Structure**:
  - Build system identification
  - Programming language detection
  - Build, test, and deploy commands
  - Reasoning explanation from LLM
  - List of detected key files
  - Warning messages for potential issues
  - Processing time metrics

#### Configuration
- Environment variable-based configuration
  - `AIPACK_BACKEND` - Backend selection (ollama, mistral, auto)
  - `AIPACK_OLLAMA_ENDPOINT` - Ollama server endpoint
  - `AIPACK_OLLAMA_MODEL` - Model selection
  - `RUST_LOG` - Logging configuration

- Default configuration presets
- Backend-specific timeout settings
- Analyzer configuration options

#### Error Handling
- Custom error types with `thiserror`:
  - `BackendError` - LLM API errors
  - `AnalysisError` - Repository scanning errors
  - `ServiceError` - High-level orchestration errors
  - `ConfigError` - Configuration validation errors

- Helpful error messages with context
- Troubleshooting hints for common issues
- Automatic error recovery where possible

#### Logging
- Structured logging with `tracing`
- Configurable log levels (debug, info, warn, error)
- Pretty and JSON output formats
- Module-level filtering
- Performance event tracking

#### Documentation
- **README.md** - Comprehensive user guide with quick start, usage examples, FAQ
- **docs/ARCHITECTURE.md** - System design, components, data flow, and patterns
- **docs/DEVELOPMENT.md** - Developer guide for contributors
- **docs/EXAMPLES.md** - Real-world usage examples and integration patterns
- **docs/TROUBLESHOOTING.md** - Common issues and solutions
- **docs/CONFIGURATION_GUIDE.md** - Complete configuration reference
- **CONTRIBUTING.md** - Contribution guidelines and standards
- **PRD.md** - Product requirements and vision
- **CLAUDE.md** - AI assistant development guide

#### Examples
- **basic_detect.rs** - Simple detection example (~100 lines)
- **custom_config.rs** - Custom configuration and backend comparison (~150 lines)
- **batch_analyze.rs** - Batch repository analysis with reporting (~200 lines)
- **advanced_workflow.rs** - Production-ready workflow with error handling (~250 lines)
- **analyze_repository.rs** - Repository analyzer demonstration
- **basic_config.rs** - Configuration example
- **logging_demo.rs** - Logging setup demonstration
- **ollama_detect.rs** - Ollama-specific detection example

#### Testing
- Comprehensive unit tests for all modules
- Integration tests for Ollama backend
- Documentation tests in code examples
- Test fixtures for common repository types
- Mock backend for testing without LLM

#### Build & Packaging
- Optimized release builds with LTO
- Cross-platform support (Linux, macOS, Windows)
- Binary size optimization
- Reproducible builds

### Changed
- N/A (Initial release)

### Deprecated
- N/A (Initial release)

### Removed
- N/A (Initial release)

### Fixed
- N/A (Initial release)

### Security
- API keys loaded from environment only (never hardcoded)
- No secrets in logs or error messages
- Path validation to prevent traversal
- File size limits to prevent DoS
- Input sanitization for LLM prompts

## Release Statistics

### Code Metrics
- **Lines of Code**: ~5,000+ (excluding tests and examples)
- **Test Coverage**: ~80% (unit + integration)
- **Dependencies**: 13 runtime, 2 dev
- **Modules**: 15 source modules
- **Examples**: 8 runnable examples
- **Documentation**: 2,500+ lines

### Performance
- **Detection Time**: 1-8 seconds (depending on model and repository size)
- **Binary Size**: ~8MB (release build)
- **Memory Usage**: 50-100MB (excluding LLM model)
- **Supported Platforms**: Linux, macOS, Windows

### Language Support
Tested with projects in:
- Rust (cargo)
- JavaScript/TypeScript (npm, yarn, pnpm)
- Java (maven, gradle)
- Python (pip, poetry, pipenv)
- Go (go mod)
- Ruby (bundler)
- PHP (composer)
- .NET (dotnet)
- And many more

## Migration Guide

### From Development to 0.1.0
This is the first release, no migration needed.

## Known Issues

### Limitations
- Claude and OpenAI backends not yet implemented (coming in Phase 2)
- No built-in result caching (workaround: implement manually, see docs/EXAMPLES.md)
- Monorepo detection requires analyzing subdirectories individually
- Large repositories (>500 files) may exceed context limits

### Workarounds
- **Slow detection**: Use qwen2.5-coder:7b instead of qwen:14b
- **Low confidence**: Try more powerful model or verify results manually
- **Ollama not available**: Install and start Ollama, or use Mistral API
- **Large repositories**: Analyze subdirectories separately

## Acknowledgments

### Contributors
- Initial implementation and architecture
- Comprehensive documentation
- Example programs and tests

### Dependencies
- **tokio** - Async runtime
- **clap** - CLI argument parsing
- **serde/serde_json** - Serialization
- **tracing** - Structured logging
- **reqwest** - HTTP client
- **anyhow** - Error handling
- **thiserror** - Custom error types

### Inspiration
- Buildpacks (Cloud Native Buildpacks project)
- AI-powered developer tools
- Modern build system complexity

## Future Roadmap

### Short Term (Next 3 months)
- Claude API backend
- OpenAI GPT backend
- Result caching
- Performance benchmarks

### Medium Term (3-6 months)
- HTTP/REST API service
- Web UI dashboard
- Batch processing capabilities
- Community feedback integration

### Long Term (6-12 months)
- Web UI
- Plugin system
- Learning from user corrections
- Build command validation
- Interactive detection mode

## Notes

This is the initial MVP release of aipack. The core detection workflow is stable and production-ready for individual repository analysis. Advanced features like caching, multiple cloud backends, and platform integration are planned for future releases.

We welcome feedback, bug reports, and contributions! Please see CONTRIBUTING.md for guidelines.

---

For detailed release notes and migration guides, see individual version sections above.

For support, please visit:
- GitHub Issues: https://github.com/diverofdark/aipack/issues
- GitHub Discussions: https://github.com/diverofdark/aipack/discussions
- Documentation: docs/ directory in the repository
