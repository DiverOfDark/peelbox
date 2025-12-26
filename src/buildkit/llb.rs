use anyhow::{Context as AnyhowContext, Result};
use buildkit_llb::prelude::*;
use crate::output::schema::UniversalBuild;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
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
    /// Stage 2 (Runtime Prep):
    ///   6. Install runtime packages on wolfi-base
    ///   7. Remove apk tooling (/sbin/apk, /etc/apk, /lib/apk, /var/cache/apk)
    /// Stage 3 (Squash to Clean Base):
    ///   8. Start with glibc-dynamic (clean base, no apk in history)
    ///   9. Copy all files from runtime prep (cp -a /source/. /)
    ///   Result: Single squashed layer with packages but no apk in history
    /// Stage 4 (Final):
    ///   10. Copy artifacts from build stage
    ///   11. Set command and environment
    /// Result: No apk in any layer, truly distroless
    pub fn write_definition<W: Write>(
        &self,
        spec: &UniversalBuild,
        writer: W,
    ) -> Result<()> {
        // Create all sources first
        let base = Source::image(WOLFI_BASE_IMAGE);

        // Load gitignore patterns and apply to context source
        let exclude_patterns = Self::load_gitignore_patterns();
        let mut context = Source::local(&self.context_name);
        for pattern in exclude_patterns {
            context = context.add_exclude_pattern(pattern);
        }

        let runtime_base = Source::image(WOLFI_BASE_IMAGE);
        let glibc_dynamic = Source::image("cgr.dev/chainguard/glibc-dynamic:latest");

        // Stage 1: Build stage
        let with_packages = if !spec.build.packages.is_empty() {
            let packages = spec.build.packages.join(" ");
            let cmd = format!("apk add --no-cache {}", packages);
            Some(
                Command::run("sh")
                    .args(&["-c", &cmd])
                    .mount(Mount::Layer(OutputIdx(0), base.output(), "/"))
                    .mount(Mount::Scratch(OutputIdx(1), "/tmp"))
                    .custom_name("Install build packages"),
            )
        } else {
            None
        };

        let build_stage = {
            let mut build_cmd = Command::run("sh");

            if let Some(ref pkg_cmd) = with_packages {
                build_cmd = build_cmd.mount(Mount::ReadOnlyLayer(pkg_cmd.output(0), "/"));
            } else {
                build_cmd = build_cmd.mount(Mount::ReadOnlyLayer(base.output(), "/"));
            }

            build_cmd = build_cmd
                .mount(Mount::Layer(OutputIdx(0), context.output(), "/build"))
                .mount(Mount::Scratch(OutputIdx(1), "/tmp"))
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

            // Set environment variables (resolve relative paths to /build)
            for (key, value) in &spec.build.env {
                let resolved_value = if value.starts_with('/') || value.starts_with('$') {
                    value.clone()
                } else if value.starts_with('.') {
                    format!("/build/{}", value)
                } else {
                    value.clone()
                };
                build_cmd = build_cmd.env(key, &resolved_value);
            }

            if !spec.build.commands.is_empty() {
                // Build commands + copy artifacts out of cache mounts
                let mut script_parts = vec![spec.build.commands.join(" && ")];

                // Copy artifacts from cache mounts to output filesystem
                if !spec.build.artifacts.is_empty() {
                    script_parts.push("mkdir -p /tmp/artifacts".to_string());
                    for artifact in &spec.build.artifacts {
                        script_parts.push(format!("cp {} /tmp/artifacts/", artifact));
                    }
                }

                let script = script_parts.join(" && ");
                build_cmd = build_cmd.args(&["-c", &script]);
                build_cmd = build_cmd.custom_name("Run build commands");
            } else {
                build_cmd = build_cmd.args(&["-c", "true"]);
                build_cmd = build_cmd.custom_name("Build stage");
            }

            build_cmd
        };

        // Stage 2: Runtime Prep (install runtime packages + remove apk)
        let runtime_prep = if !spec.runtime.packages.is_empty() {
            let packages = spec.runtime.packages.join(" ");
            let cmd = format!(
                "apk add --no-cache {} && rm -rf /sbin/apk /etc/apk /lib/apk /var/cache/apk",
                packages
            );
            Command::run("sh")
                .args(&["-c", &cmd])
                .mount(Mount::Layer(OutputIdx(0), runtime_base.output(), "/"))
                .mount(Mount::Scratch(OutputIdx(1), "/tmp"))
                .custom_name("Install runtime packages and remove apk")
        } else {
            // No runtime packages - just use wolfi-base and remove apk
            Command::run("sh")
                .args(&["-c", "rm -rf /sbin/apk /etc/apk /lib/apk /var/cache/apk"])
                .mount(Mount::Layer(OutputIdx(0), runtime_base.output(), "/"))
                .custom_name("Remove apk from base")
        };

        // Stage 3: Squash to clean glibc-dynamic base (no apk in history)
        // Use busybox to copy all files from runtime prep onto glibc-dynamic
        // (can't use sh on glibc-dynamic itself as it's distroless)
        let runtime_desc = if !spec.runtime.packages.is_empty() {
            format!("aipack {} runtime", spec.runtime.packages.join(" "))
        } else {
            "aipack base runtime".to_string()
        };
        let busybox = Source::image("cgr.dev/chainguard/busybox:latest");
        let squashed_runtime = Command::run("sh")
            .args(&["-c", &format!(": {}; cp -a /source/. /dest/", runtime_desc)])
            .mount(Mount::ReadOnlyLayer(busybox.output(), "/"))
            .mount(Mount::Layer(OutputIdx(0), glibc_dynamic.output(), "/dest"))
            .mount(Mount::ReadOnlyLayer(runtime_prep.output(0), "/source"))
            .custom_name(&runtime_desc);

        // Stage 4: Copy artifacts from build stage onto squashed runtime base
        let mut final_stage = Command::run("sh")
            .mount(Mount::Layer(OutputIdx(0), squashed_runtime.output(0), "/"))
            .mount(Mount::ReadOnlyLayer(build_stage.output(1), "/tmp/build-tmp"));

        let app_name = spec.metadata.project_name.as_deref().unwrap_or("app");

        if !spec.runtime.copy.is_empty() {
            let mut copy_commands = vec![format!(": aipack {} application", app_name)];
            for copy_spec in &spec.runtime.copy {
                let filename = copy_spec.from.split('/').last().unwrap_or(&copy_spec.from);
                // Create parent directory if needed
                let parent_dir = copy_spec.to.rsplitn(2, '/').nth(1);
                if let Some(dir) = parent_dir {
                    if !dir.is_empty() {
                        copy_commands.push(format!("mkdir -p {}", dir));
                    }
                }
                copy_commands.push(format!("cp /tmp/build-tmp/artifacts/{} {}", filename, copy_spec.to));
            }
            let script = copy_commands.join(" && ");
            final_stage = final_stage.args(&["-c", &script]);
            final_stage = final_stage.custom_name(&format!("aipack {} application", app_name));
        } else {
            final_stage = final_stage.args(&["-c", &format!(": aipack {} application", app_name)]);
            final_stage = final_stage.custom_name(&format!("aipack {} application", app_name));
        }

        for (key, value) in &spec.runtime.env {
            final_stage = final_stage.env(key, value);
        }

        Terminal::with(final_stage.output(0))
            .write_definition(writer)
            .with_context(|| "Failed to write LLB definition")?;

        Ok(())
    }

    /// Generate LLB definition as bytes
    pub fn build(&self, spec: &UniversalBuild) -> Result<Vec<u8>> {
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
                confidence: 1.0,
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
                artifacts: vec!["/build/target/release/app".to_string()],
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

        let result = builder.build(&spec);
        assert!(result.is_ok(), "Full build should succeed");

        let bytes = result.unwrap();
        assert!(!bytes.is_empty(), "Should generate non-empty LLB definition");
    }

    #[test]
    fn test_empty_packages() {
        let builder = LLBBuilder::new("context");
        let mut spec = create_test_spec();
        spec.build.packages.clear();

        let result = builder.build(&spec);
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

        let result = builder.build(&spec);
        assert!(result.is_ok(), "Build with env vars should succeed");
    }
}
