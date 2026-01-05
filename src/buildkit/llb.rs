use crate::output::schema::UniversalBuild;
use anyhow::{Context as AnyhowContext, Result};
use buildkit_llb::prelude::*;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::debug;

const WOLFI_BASE_IMAGE: &str = "cgr.dev/chainguard/wolfi-base:latest";

pub struct LLBBuilder {
    context_name: String,
}

impl LLBBuilder {
    pub fn new(context_name: impl Into<String>) -> Self {
        Self {
            context_name: context_name.into(),
        }
    }

    /// Normalize path for directory detection: "." becomes "./"
    fn normalize_path(path: &str) -> &str {
        if path == "." {
            "./"
        } else {
            path
        }
    }

    /// Check if path represents a directory
    fn is_directory(path: &str) -> bool {
        path.ends_with('/')
    }

    /// Read .gitignore and parse exclude patterns
    fn load_gitignore_patterns() -> Vec<String> {
        let gitignore_path = PathBuf::from(".gitignore");

        let mut patterns = Vec::new();

        // Read .gitignore if it exists
        if gitignore_path.exists() {
            if let Ok(content) = fs::read_to_string(&gitignore_path) {
                for line in content.lines() {
                    let line = line.trim();
                    // Skip empty lines and comments
                    if !line.is_empty() && !line.starts_with('#') {
                        patterns.push(line.to_string());
                    }
                }
                debug!("Loaded {} patterns from .gitignore", patterns.len());
            }
        }

        // Add standard exclusions
        patterns.extend(vec![
            ".git".to_string(),
            ".gitignore".to_string(),
            ".dockerignore".to_string(),
            "*.md".to_string(),
            "LICENSE".to_string(),
            ".vscode".to_string(),
            ".idea".to_string(),
            "*.swp".to_string(),
            "*.swo".to_string(),
            "*~".to_string(),
            ".DS_Store".to_string(),
        ]);

        debug!("Total exclude patterns: {}", patterns.len());
        patterns
    }

    /// Generate complete LLB definition and write to output
    /// This generates a 4-stage distroless build with squashed runtime:
    /// Stage 1 (Build):
    ///   1. Starts with wolfi-base
    ///   2. Installs build packages
    ///   3. Copies source context
    ///   4. Runs build commands
    ///   5. Outputs artifacts to /tmp/artifacts
    ///
    ///      Stage 2 (Runtime Prep):
    ///   6. Install runtime packages on wolfi-base
    ///   7. Remove apk tooling (/sbin/apk, /etc/apk, /lib/apk, /var/cache/apk)
    ///
    ///      Stage 3 (Squash to Clean Base):
    ///   8. Start with glibc-dynamic (clean base, no apk in history)
    ///   9. Copy all files from runtime prep (cp -a /source/. /)
    ///
    ///      Result: Single squashed layer with packages but no apk in history
    ///
    ///      Stage 4 (Final):
    ///   10. Copy artifacts from build stage
    ///   11. Set command and environment
    ///
    ///       Result: No apk in any layer, truly distroless
    pub fn write_definition<W: Write>(&self, spec: &UniversalBuild, writer: W) -> Result<()> {
        // Create all sources first
        let wolfi_base = Source::image(WOLFI_BASE_IMAGE);
        let glibc_dynamic = Source::image("cgr.dev/chainguard/glibc-dynamic:latest");
        let busybox = Source::image("cgr.dev/chainguard/busybox:latest");

        // Load gitignore patterns and apply to context source
        let exclude_patterns = Self::load_gitignore_patterns();
        let mut context = Source::local(&self.context_name);
        for pattern in exclude_patterns {
            context = context.add_exclude_pattern(pattern);
        }

        // Stage 1: Build stage
        let with_packages = if !spec.build.packages.is_empty() {
            let packages = spec.build.packages.join(" ");
            let cmd = format!("apk add --no-cache {}", packages);
            Some(
                Command::run("sh")
                    .args(["-c", &cmd])
                    .mount(Mount::Layer(OutputIdx(0), wolfi_base.output(), "/"))
                    .mount(Mount::Scratch(OutputIdx(1), "/tmp"))
                    .custom_name("Install build packages"),
            )
        } else {
            None
        };

        let build_stage = {
            let mut build_cmd = Command::run("sh");

            if let Some(ref pkg_cmd) = with_packages {
                build_cmd = build_cmd.mount(Mount::Layer(OutputIdx(0), pkg_cmd.output(0), "/"));
            } else {
                build_cmd = build_cmd.mount(Mount::Layer(OutputIdx(0), wolfi_base.output(), "/"));
            }

            build_cmd = build_cmd
                .mount(Mount::Layer(OutputIdx(1), context.output(), "/build"))
                .mount(Mount::Scratch(OutputIdx(2), "/tmp"))
                .cwd("/build");

            // Add cache mounts for build system caches (resolve relative to /build)
            for cache_path in &spec.build.cache {
                let absolute_cache_path = if cache_path.starts_with('/') {
                    cache_path.clone()
                } else {
                    format!("/build/{}", cache_path)
                };
                build_cmd = build_cmd.mount(Mount::SharedCache(&absolute_cache_path));
            }

            // Execute each build command as a separate layer for better caching
            // Use Arc to keep Commands alive with stable references
            let mut build_stages: Vec<Arc<Command>> = Vec::new();

            if !spec.build.commands.is_empty() {
                for (idx, command) in spec.build.commands.iter().enumerate() {
                    // For first command, copy context to /build in root fs
                    let build_script = if idx == 0 {
                        format!(
                            "mkdir -p /build && cp -r /context/. /build && cd /build && {}",
                            command
                        )
                    } else {
                        command.clone()
                    };

                    let mut cmd = Command::run("sh").args(["-c", &build_script]).cwd("/build");

                    // Mount base layer (first command) or previous command's output
                    if idx == 0 {
                        if let Some(ref pkg_cmd) = with_packages {
                            cmd = cmd.mount(Mount::Layer(OutputIdx(0), pkg_cmd.output(0), "/"));
                        } else {
                            cmd = cmd.mount(Mount::Layer(OutputIdx(0), wolfi_base.output(), "/"));
                        }
                    } else {
                        // Reference previous command via Arc
                        cmd = cmd.mount(Mount::Layer(
                            OutputIdx(0),
                            build_stages[idx - 1].output(0),
                            "/",
                        ));
                    }

                    // Mount context read-only for first command, /tmp scratch for all
                    if idx == 0 {
                        cmd = cmd
                            .mount(Mount::ReadOnlyLayer(context.output(), "/context"))
                            .mount(Mount::Scratch(OutputIdx(1), "/tmp"));
                    } else {
                        cmd = cmd.mount(Mount::Scratch(OutputIdx(1), "/tmp"));
                    }

                    // Add cache mounts
                    for cache_path in &spec.build.cache {
                        let absolute_cache_path = if cache_path.starts_with('/') {
                            cache_path.clone()
                        } else {
                            format!("/build/{}", cache_path)
                        };
                        cmd = cmd.mount(Mount::SharedCache(&absolute_cache_path));
                    }

                    // Set environment variables
                    for (key, value) in &spec.build.env {
                        let resolved_value = if value.starts_with('/') || value.starts_with('$') {
                            value.clone()
                        } else if value.starts_with('.') {
                            format!("/build/{}", value)
                        } else {
                            value.clone()
                        };
                        cmd = cmd.env(key, &resolved_value);
                    }

                    cmd = cmd.custom_name(format!("Build command {}", idx + 1));
                    build_stages.push(Arc::new(cmd));
                }
            }

            // Extract artifacts from runtime.copy[].from
            let artifacts: Vec<&String> = spec
                .runtime
                .copy
                .iter()
                .map(|copy_spec| &copy_spec.from)
                .collect();

            // Final layer: Copy artifacts out of cache mounts (after all build commands)
            let build_stage = if !artifacts.is_empty() && !build_stages.is_empty() {
                let mut artifact_cmd = Command::run("sh").cwd("/build");

                // Mount last build command's output (which includes /build directory)
                artifact_cmd = artifact_cmd
                    .mount(Mount::Layer(
                        OutputIdx(0),
                        build_stages.last().unwrap().output(0),
                        "/",
                    ))
                    .mount(Mount::Scratch(OutputIdx(1), "/tmp"));

                // Add cache mounts
                for cache_path in &spec.build.cache {
                    let absolute_cache_path = if cache_path.starts_with('/') {
                        cache_path.clone()
                    } else {
                        format!("/build/{}", cache_path)
                    };
                    artifact_cmd = artifact_cmd.mount(Mount::SharedCache(&absolute_cache_path));
                }

                // Build artifact copy script from runtime.copy sources
                let mut copy_commands = vec!["mkdir -p /tmp/artifacts".to_string()];
                for artifact in &artifacts {
                    let normalized_artifact = Self::normalize_path(artifact);

                    if Self::is_directory(normalized_artifact) {
                        // Remove trailing slash to copy directory itself, not contents
                        let trimmed = normalized_artifact.trim_end_matches('/');
                        copy_commands.push(format!("cp -r {} /tmp/artifacts/", trimmed));
                    } else {
                        copy_commands.push(format!("cp {} /tmp/artifacts/", normalized_artifact));
                    }
                }

                let script = copy_commands.join(" && ");
                artifact_cmd
                    .args(["-c", &script])
                    .custom_name("Copy build artifacts")
            } else if !build_stages.is_empty() {
                // No artifacts to copy, use last build command
                // Create a no-op command that passes through the build stage
                Command::run("sh")
                    .args(["-c", "true"])
                    .mount(Mount::Layer(
                        OutputIdx(0),
                        build_stages.last().unwrap().output(0),
                        "/",
                    ))
                    .custom_name("Build stage (passthrough)")
            } else {
                // No commands at all, create empty build stage
                build_cmd.args(["-c", "true"]).custom_name("Build stage")
            };

            build_stage
        };

        // Stage 2: Runtime Prep (install runtime packages + remove apk)
        let runtime_prep = if !spec.runtime.packages.is_empty() {
            let packages = spec.runtime.packages.join(" ");
            let cmd = format!(
                "apk add --no-cache {} && rm -rf /sbin/apk /etc/apk /lib/apk /var/cache/apk",
                packages
            );
            Command::run("sh")
                .args(["-c", &cmd])
                .mount(Mount::Layer(OutputIdx(0), wolfi_base.output(), "/"))
                .mount(Mount::Scratch(OutputIdx(1), "/tmp"))
                .custom_name("Install runtime packages and remove apk")
        } else {
            // No runtime packages - just use wolfi-base and remove apk
            Command::run("sh")
                .args(["-c", "rm -rf /sbin/apk /etc/apk /lib/apk /var/cache/apk"])
                .mount(Mount::Layer(OutputIdx(0), wolfi_base.output(), "/"))
                .custom_name("Remove apk from base")
        };

        // Stage 3: Squash to clean glibc-dynamic base (no apk in history)
        // Use busybox to copy all files from runtime prep onto glibc-dynamic
        // (can't use sh on glibc-dynamic itself as it's distroless)
        let runtime_desc = if !spec.runtime.packages.is_empty() {
            format!("peelbox {} runtime", spec.runtime.packages.join(" "))
        } else {
            "peelbox base runtime".to_string()
        };
        let squashed_runtime = Command::run("sh")
            .args(["-c", &format!(": {}; cp -a /source/. /dest/", runtime_desc)])
            .mount(Mount::ReadOnlyLayer(busybox.output(), "/"))
            .mount(Mount::Layer(OutputIdx(0), glibc_dynamic.output(), "/dest"))
            .mount(Mount::ReadOnlyLayer(runtime_prep.output(0), "/source"))
            .custom_name(&runtime_desc);

        // Stage 4: Copy artifacts from build stage onto squashed runtime base
        let app_name = spec.metadata.project_name.as_deref().unwrap_or("app");

        if !spec.runtime.copy.is_empty() {
            // Use busybox for shell commands since squashed runtime is distroless
            let mut copy_stage = Command::run("/bin/sh")
                .mount(Mount::ReadOnlyLayer(busybox.output(), "/"))
                .mount(Mount::Layer(
                    OutputIdx(0),
                    squashed_runtime.output(0),
                    "/dest",
                ))
                .mount(Mount::ReadOnlyLayer(
                    build_stage.output(1),
                    "/tmp/build-tmp",
                ));

            // Set runtime environment variables
            for (key, value) in &spec.runtime.env {
                copy_stage = copy_stage.env(key, value);
            }

            let mut copy_commands = vec![format!(": peelbox {} application", app_name)];
            for copy_spec in &spec.runtime.copy {
                let normalized_from = Self::normalize_path(&copy_spec.from);

                let source_path = if Self::is_directory(normalized_from) {
                    normalized_from.trim_end_matches('/')
                } else {
                    normalized_from
                };
                let filename = source_path.split('/').next_back().unwrap_or(source_path);

                // Create parent directory if needed (in /dest)
                let parent_dir = copy_spec.to.rsplit_once('/').map(|x| x.0);
                if let Some(dir) = parent_dir {
                    if !dir.is_empty() {
                        copy_commands.push(format!("/bin/mkdir -p /dest{}", dir));
                    }
                }

                let copy_flag = if Self::is_directory(normalized_from) {
                    "-r"
                } else {
                    ""
                };
                copy_commands.push(format!(
                    "/bin/cp {} /tmp/build-tmp/artifacts/{} /dest{}",
                    copy_flag, filename, copy_spec.to
                ));
            }
            let script = copy_commands.join(" && ");
            copy_stage = copy_stage.args(["-c", &script]);
            copy_stage = copy_stage.custom_name(format!("peelbox {} application", app_name));

            Terminal::with(copy_stage.output(0))
                .write_definition(writer)
                .with_context(|| "Failed to write LLB definition")?;
        } else {
            // No artifacts to copy - use squashed runtime directly
            Terminal::with(squashed_runtime.output(0))
                .write_definition(writer)
                .with_context(|| "Failed to write LLB definition")?;
        }

        Ok(())
    }

    /// Generate LLB definition as bytes for gRPC submission
    pub fn to_bytes(&self, spec: &UniversalBuild) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        self.write_definition(spec, &mut buffer)?;
        Ok(buffer)
    }
}

impl Default for LLBBuilder {
    fn default() -> Self {
        Self::new("context")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::schema::{BuildMetadata, BuildStage, RuntimeStage};
    use std::collections::HashMap;

    fn create_test_spec() -> UniversalBuild {
        UniversalBuild {
            version: "1.0".to_string(),
            metadata: BuildMetadata {
                project_name: Some("test-app".to_string()),
                language: "rust".to_string(),
                build_system: "cargo".to_string(),
                framework: None,
                reasoning: "Test".to_string(),
            },
            build: BuildStage {
                packages: vec!["rust".to_string(), "build-base".to_string()],
                env: {
                    let mut map = HashMap::new();
                    map.insert("CARGO_HOME".to_string(), "/cache/cargo".to_string());
                    map
                },
                commands: vec!["cargo build --release".to_string()],
                cache: vec!["/cache/cargo".to_string()],
            },
            runtime: RuntimeStage {
                packages: vec![],
                env: HashMap::new(),
                copy: vec![],
                command: vec!["./app".to_string()],
                ports: vec![],
                health: None,
            },
        }
    }

    #[test]
    fn test_llb_builder_creation() {
        let builder = LLBBuilder::new("context");
        assert_eq!(builder.context_name, "context");
    }

    #[test]
    fn test_full_build() {
        let builder = LLBBuilder::new("context");
        let spec = create_test_spec();

        let result = builder.to_bytes(&spec);
        assert!(result.is_ok(), "Full build should succeed");

        let bytes = result.unwrap();
        assert!(
            !bytes.is_empty(),
            "Should generate non-empty LLB definition"
        );
    }

    #[test]
    fn test_empty_packages() {
        let builder = LLBBuilder::new("context");
        let mut spec = create_test_spec();
        spec.build.packages.clear();

        let result = builder.to_bytes(&spec);
        assert!(
            result.is_ok(),
            "Should handle empty packages list gracefully"
        );
    }

    #[test]
    fn test_default_builder() {
        let builder = LLBBuilder::default();
        assert_eq!(builder.context_name, "context");
    }

    #[test]
    fn test_with_environment_variables() {
        let builder = LLBBuilder::new("context");
        let spec = create_test_spec();

        assert!(!spec.build.env.is_empty(), "Test spec should have env vars");

        let result = builder.to_bytes(&spec);
        assert!(result.is_ok(), "Build with env vars should succeed");
    }
}
