//! Map-Reduce detection pipeline.
//!
//! Four steps: Parse → Detect Framework → Partition → Reduce.
//!
//! Build-system-specific behavior is delegated to `BuildSystemConfig` profiles
//! registered via `inventory::submit!` in the parser files.

pub(crate) use crate::helpers::{extract_project_dir, replace_package};
pub(crate) use crate::parsers::config::mise::scan_mise_config;
pub(crate) use crate::parsers::manifest::cargo_toml::{read_rust_version, resolve_rust_toolchain};
pub(crate) use crate::parsers::manifest::composer_json::read_php_version;
pub(crate) use crate::parsers::manifest::gemfile::read_ruby_version;
pub(crate) use crate::parsers::manifest::package_json::{
    detect_react_router_spa, ensure_node_tooling_floor, ensure_npm_node_gyp,
    ensure_pnpm_allow_builds, provide_framework_fallback_entrypoint, read_node_version,
    resolve_node_version, sanitize_node_build_commands, scan_node_native_deps, scan_node_puppeteer,
    wrap_yarn_corepack_entrypoint,
};
pub(crate) use crate::parsers::manifest::pom_xml::{
    resolve_java_toolchain, sync_java_home_with_packages,
};
pub(crate) use crate::parsers::manifest::pyproject_toml::{
    fix_django_settings, fix_flask_app_path, read_python_version, scan_python_entrypoints,
    scan_python_native_deps,
};
pub(crate) use crate::registry::Registry;
pub(crate) use crate::source_scanning::{
    scan_source_env_vars, scan_source_health, scan_source_ports,
};
pub(crate) use crate::traits::{ConfigParser, ManifestParser};
pub(crate) use crate::types::*;

pub(crate) use anyhow::Result;
pub(crate) use ignore::WalkBuilder;
pub(crate) use peelbox_core::output::schema::{
    BuildMetadata, BuildStage, CopySpec, HealthCheck, RuntimeStage, UniversalBuild,
};
pub(crate) use peelbox_wolfi::WolfiPackageIndex;
pub(crate) use std::collections::HashMap;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use tracing::{debug, info, warn};

/// Run the full detection pipeline on a repository.
/// Resolves Wolfi package versions automatically.
pub fn detect(repo_path: &Path) -> Result<Vec<UniversalBuild>> {
    let registry = Registry::with_defaults();
    let wolfi_index = WolfiPackageIndex::fetch()?;
    detect_with_registry_and_wolfi(repo_path, &registry, Some(&wolfi_index))
}

/// Run without Wolfi resolution (for testing).
pub fn detect_without_wolfi(repo_path: &Path) -> Result<Vec<UniversalBuild>> {
    let registry = Registry::with_defaults();
    detect_with_registry_and_wolfi(repo_path, &registry, None)
}

/// Run the full detection pipeline with a custom registry.
pub fn detect_with_registry(repo_path: &Path, registry: &Registry) -> Result<Vec<UniversalBuild>> {
    detect_with_registry_and_wolfi(repo_path, registry, None)
}

/// Run the full detection pipeline with a custom registry and optional Wolfi index.
pub fn detect_with_registry_and_wolfi(
    repo_path: &Path,
    registry: &Registry,
    wolfi_index: Option<&WolfiPackageIndex>,
) -> Result<Vec<UniversalBuild>> {
    info!(repo = %repo_path.display(), "Starting map-reduce detection pipeline");

    // Step 1: Walk filesystem and parse into typed tree
    let repo_tree = build_tree(repo_path, registry)?;

    // Step 2: Framework detection on all manifests
    let manifests_with_frameworks = detect_frameworks(&repo_tree, registry);

    // Step 3: Partition into service buckets
    let buckets = partition(&repo_tree, manifests_with_frameworks, registry);

    info!(services = buckets.len(), "Partitioned into service buckets");

    // Step 4: Reduce each bucket into UniversalBuild
    let mut builds: Vec<UniversalBuild> = buckets
        .into_iter()
        .filter_map(|bucket| match reduce(bucket, registry) {
            Ok(build) => Some(build),
            Err(e) => {
                warn!("Failed to reduce service bucket: {}", e);
                None
            }
        })
        .collect();

    // Step 4b: Scan source code for port patterns
    for build in &mut builds {
        scan_source_ports(repo_path, build);
    }

    // Step 4c: Scan source code for health endpoints
    for build in &mut builds {
        scan_source_health(repo_path, build);
    }

    // Step 4d: Scan source code for environment variables
    for build in &mut builds {
        scan_source_env_vars(repo_path, build);
    }

    // Step 4e: Scan version files (.nvmrc, .python-version, etc.)
    for build in &mut builds {
        scan_version_files(repo_path, build);
    }

    // Step 4e2: Scan mise.toml / .mise.toml / .tool-versions for tool version overrides
    for build in &mut builds {
        scan_mise_config(repo_path, build);
    }

    // Step 4e3: Floor the Node.js version to what the latest pnpm/npm/yarn
    // packages (the only versions Wolfi ships) actually support.
    for build in &mut builds {
        ensure_node_tooling_floor(build);
    }

    // Step 4f: Scan Python entrypoints
    for build in &mut builds {
        scan_python_entrypoints(repo_path, build);
    }

    // Step 4g: Fix FLASK_APP for workspace members where hardcoded path is wrong
    for build in &mut builds {
        fix_flask_app_path(repo_path, build);
    }

    // Step 4h: Detect Django settings module from manage.py
    for build in &mut builds {
        fix_django_settings(repo_path, build);
    }

    // Step 4i: Detect Python native dependency system packages
    for build in &mut builds {
        scan_python_native_deps(repo_path, build);
    }

    // Step 4j: Ensure npm builds have node-gyp available (Wolfi npm doesn't bundle it)
    for build in &mut builds {
        ensure_npm_node_gyp(build);
    }

    // Step 4k: Detect Node.js native dependency system packages
    for build in &mut builds {
        scan_node_native_deps(repo_path, build);
    }

    // Step 4l: Detect Puppeteer and add Chromium dependencies
    for build in &mut builds {
        scan_node_puppeteer(repo_path, build);
    }

    // Step 4m: Sanitize Node.js build commands (remove DB-dependent steps, etc.)
    for build in &mut builds {
        sanitize_node_build_commands(repo_path, build);
    }

    // Step 4n: Approve pnpm dependency build scripts (pnpm 10.16+/11 otherwise
    // fails `pnpm install` with ERR_PNPM_IGNORED_BUILDS).
    for build in &mut builds {
        ensure_pnpm_allow_builds(build);
    }

    // Step 5: Resolve Wolfi package versions
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            resolve_wolfi_packages(&mut build.build.packages, wolfi);
            resolve_wolfi_packages(&mut build.runtime.packages, wolfi);
        }
    }

    // Step 5b: Sync JAVA_HOME with resolved openjdk package version
    for build in &mut builds {
        sync_java_home_with_packages(build);
    }

    // Step 6: Handle pinned versions not available in Wolfi (use alternative installers)
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            resolve_rust_toolchain(build, wolfi);
            resolve_node_version(build, wolfi);
        }
    }

    // Step 7: Handle old Java versions not available in Wolfi (use Adoptium Temurin)
    if let Some(wolfi) = wolfi_index {
        for build in &mut builds {
            resolve_java_toolchain(build, wolfi);
        }
    }

    // Step 7b: Set PORT env var for JVM apps when runtime has ports.
    // Many Spring/JVM apps use ${PORT:default} in config; PaaS convention is to set PORT.
    for build in &mut builds {
        let lang = build.metadata.language.as_str();
        if matches!(lang, "Java" | "Kotlin" | "Scala" | "Clojure") {
            if let Some(&port) = build.runtime.ports.first() {
                build
                    .runtime
                    .env
                    .entry("PORT".to_string())
                    .or_insert_with(|| port.to_string());
            }
        }
    }

    // Step 8: Wrap Yarn Berry entrypoints with corepack enable
    for build in &mut builds {
        wrap_yarn_corepack_entrypoint(build);
    }

    // Step 9: Provide fallback entrypoints for detected frameworks without scripts.start
    for build in &mut builds {
        provide_framework_fallback_entrypoint(build);
    }

    // Step 9b: Detect React Router SPA mode and switch to vite preview
    for build in &mut builds {
        detect_react_router_spa(repo_path, build);
    }

    // Filter out non-application builds (e.g., library crates, utility packages)
    builds.retain(|b| !b.runtime.command.is_empty());

    info!(builds = builds.len(), "Detection pipeline complete");
    Ok(builds)
}

// ── Pipeline stages (extracted into submodules) ─────────────────────────────
mod framework;
mod partitioning;
mod reducing;
mod scan;
mod tree;
mod wolfi;

use framework::*;
use partitioning::*;
use reducing::*;
use scan::*;
use tree::*;
use wolfi::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{BuildSystemId, FrameworkId, LanguageId, RuntimeId};
    use std::collections::BTreeMap;

    const RUST: LanguageId = LanguageId::new("rust");
    const JAVA: LanguageId = LanguageId::new("java");
    const CARGO: BuildSystemId = BuildSystemId::new("cargo");
    const MAVEN: BuildSystemId = BuildSystemId::new("maven");
    const NATIVE: RuntimeId = RuntimeId::new("native");
    const JVM: RuntimeId = RuntimeId::new("jvm");
    const SPRING_BOOT: FrameworkId = FrameworkId::new("spring-boot");

    #[test]
    fn test_reduce_standalone_rust() {
        let bucket = ServiceBucket {
            path: PathBuf::new(),
            manifest: Manifest {
                path: PathBuf::from("Cargo.toml"),
                language: RUST,
                build_system: CARGO,
                runtime: NATIVE,
                package: Some(Package {
                    name: "my-app".into(),
                    version: Some("0.1.0".into()),
                    is_application: true,
                }),
                workspace: None,
                dependencies: vec![],
                build: BuildSpec {
                    packages: vec!["rust".into(), "build-base".into()],
                    commands: vec!["cargo build --release".into()],
                    member_transform: None,
                    env: BTreeMap::new(),
                    cache_dirs: vec!["target".into()],
                    artifacts: vec![("target/release/my-app".into(), "/app/my-app".into())],
                },
                runtime_config: RuntimeSpec {
                    packages: vec!["ca-certificates".into()],
                    env: BTreeMap::new(),
                    entrypoint: Some("/app/my-app".into()),
                    workdir: Some("/app".into()),
                    ports: vec![8080],
                    health_endpoint: None,
                },
            },
            configs: vec![],
            framework: None,
            is_workspace_member: false,
            workspace_root: None,
        };

        let registry = Registry::with_defaults();
        let build = reduce(bucket, &registry).unwrap();
        assert_eq!(build.metadata.language, "Rust");
        assert_eq!(build.metadata.build_system, "Cargo");
        assert_eq!(build.build.commands, vec!["cargo build --release"]);
        assert_eq!(build.runtime.command, vec!["/app/my-app"]);
        assert_eq!(build.runtime.ports, vec![8080]);
    }

    #[test]
    fn test_reduce_workspace_member() {
        let bucket = ServiceBucket {
            path: PathBuf::from("api-service"),
            manifest: Manifest {
                path: PathBuf::from("api-service/pom.xml"),
                language: JAVA,
                build_system: MAVEN,
                runtime: JVM,
                package: Some(Package {
                    name: "api-service".into(),
                    version: Some("1.0.0".into()),
                    is_application: true,
                }),
                workspace: None,
                dependencies: vec![],
                build: BuildSpec {
                    packages: vec!["openjdk-21".into(), "maven".into()],
                    commands: vec!["mvn package -DskipTests".into()],
                    member_transform: Some(MemberBuildTransform {
                        member_commands: vec!["mvn -pl {module} -am install -DskipTests".into()],
                        member_artifacts: Some(vec![(
                            "{module}/target/*.jar".into(),
                            "/app/".into(),
                        )]),
                    }),
                    env: BTreeMap::new(),
                    cache_dirs: vec!["/root/.m2/repository/".into()],
                    artifacts: vec![("target/*.jar".into(), "/app/".into())],
                },
                runtime_config: RuntimeSpec {
                    packages: vec!["openjdk-21".into()],
                    env: BTreeMap::new(),
                    entrypoint: Some("java -jar /app/api-service-1.0.0.jar".into()),
                    workdir: Some("/app".into()),
                    ports: vec![8080],
                    health_endpoint: None,
                },
            },
            configs: vec![],
            framework: Some(FrameworkContribution {
                framework: SPRING_BOOT,
                default_ports: vec![8080],
                health_endpoints: vec!["/actuator/health".into()],
                env_vars: BTreeMap::new(),
                runtime_packages: vec![],
                runtime_command: None,
                runtime_env: BTreeMap::new(),
                workdir: None,
                extra_copy: vec![],
            }),
            is_workspace_member: true,
            workspace_root: Some(PathBuf::new()),
        };

        let registry = Registry::with_defaults();
        let build = reduce(bucket, &registry).unwrap();
        assert_eq!(
            build.build.commands,
            vec!["mvn -pl api-service -am install -DskipTests"]
        );
        assert_eq!(build.runtime.copy[0].from, "api-service/target/*.jar");
        assert_eq!(build.metadata.framework, Some("Spring Boot".into()));
        assert_eq!(
            build.runtime.health,
            Some(HealthCheck {
                endpoint: "/actuator/health".into()
            })
        );
    }

    #[test]
    fn test_scan_source_health_express() {
        let dir = tempfile::tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("index.js"),
            r#"
const express = require('express');
const app = express();

app.get('/healthz', (req, res) => {
    res.status(200).send('ok');
});

app.listen(3000);
"#,
        )
        .unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("test".into()),
                language: "JavaScript".into(),
                build_system: "npm".into(),
                framework: Some("Express".into()),
                reasoning: "Detected from package.json".into(),
            },
            build: BuildStage {
                packages: vec![],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
            },
            runtime: RuntimeStage {
                packages: vec![],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec!["npm".into(), "start".into()],
                workdir: "/app".into(),
                ports: vec![3000],
                health: None,
            },
        };

        scan_source_health(dir.path(), &mut build);
        assert_eq!(
            build.runtime.health,
            Some(HealthCheck {
                endpoint: "/healthz".into()
            })
        );
    }

    #[test]
    fn test_scan_source_health_does_not_override() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("index.js"),
            r#"app.get('/healthz', (req, res) => res.send('ok'));"#,
        )
        .unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("test".into()),
                language: "JavaScript".into(),
                build_system: "npm".into(),
                framework: None,
                reasoning: "Detected from package.json".into(),
            },
            build: BuildStage {
                packages: vec![],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
            },
            runtime: RuntimeStage {
                packages: vec![],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec![],
                workdir: "/app".into(),
                ports: vec![],
                health: Some(HealthCheck {
                    endpoint: "/actuator/health".into(),
                }),
            },
        };

        scan_source_health(dir.path(), &mut build);
        // Should NOT override the existing health endpoint
        assert_eq!(
            build.runtime.health,
            Some(HealthCheck {
                endpoint: "/actuator/health".into()
            })
        );
    }

    #[test]
    fn test_scan_source_env_vars_javascript() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.js"),
            r#"
const port = process.env.PORT || 3000;
const dbUrl = process.env.DATABASE_URL;
const home = process.env.HOME;
"#,
        )
        .unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("test".into()),
                language: "JavaScript".into(),
                build_system: "npm".into(),
                framework: None,
                reasoning: "Detected from package.json".into(),
            },
            build: BuildStage {
                packages: vec![],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
            },
            runtime: RuntimeStage {
                packages: vec![],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec![],
                workdir: "/app".into(),
                ports: vec![],
                health: None,
            },
        };

        scan_source_env_vars(dir.path(), &mut build);
        assert!(build.runtime.env.contains_key("PORT"));
        assert!(build.runtime.env.contains_key("DATABASE_URL"));
        // HOME is a built-in var and should be skipped
        assert!(!build.runtime.env.contains_key("HOME"));
    }

    #[test]
    fn test_extract_project_dir() {
        let root = Path::new("/repo");
        assert_eq!(
            extract_project_dir(root, "Detected from package.json in api"),
            PathBuf::from("/repo/api")
        );
        assert_eq!(
            extract_project_dir(root, "Detected from Cargo.toml"),
            PathBuf::from("/repo")
        );
    }

    #[test]
    fn test_replace_package() {
        let mut packages = vec!["nodejs".into(), "npm".into(), "ca-certificates".into()];
        replace_package(&mut packages, "nodejs", "nodejs-20");
        assert_eq!(packages, vec!["nodejs-20", "npm", "ca-certificates"]);

        // Should not replace already-versioned packages
        replace_package(&mut packages, "nodejs", "nodejs-18");
        assert_eq!(packages[0], "nodejs-20");
    }

    #[test]
    fn test_cross_language_build_merge() {
        // Test that PHP + Node.js manifests in the same directory get merged,
        // with PHP as primary and Node.js build specs appended.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Create .git directory so the walker treats it as a repo
        std::fs::create_dir_all(root.join(".git")).unwrap();

        // Create composer.json (PHP/Laravel)
        std::fs::write(
            root.join("composer.json"),
            r#"{
                "name": "laravel/laravel",
                "require": {
                    "php": "^8.2",
                    "laravel/framework": "^11.0"
                }
            }"#,
        )
        .unwrap();

        // Create package.json (Node.js/Vite for frontend assets)
        std::fs::write(
            root.join("package.json"),
            r#"{
                "private": true,
                "scripts": { "build": "vite build" },
                "dependencies": { "react": "^18.2.0" },
                "devDependencies": { "vite": "^5.0.0" }
            }"#,
        )
        .unwrap();

        let results = detect_without_wolfi(root).unwrap();

        // Should produce exactly one service (merged)
        assert_eq!(results.len(), 1);
        let build = &results[0];

        // Primary should be PHP/Composer (alphabetically first, server-side language)
        assert_eq!(build.metadata.language, "PHP");
        assert_eq!(build.metadata.build_system, "Composer");
        assert_eq!(build.metadata.framework.as_deref(), Some("Laravel"));

        // Build commands should include both Composer and npm commands
        assert!(
            build
                .build
                .commands
                .iter()
                .any(|c| c.contains("composer install")),
            "Should have composer command"
        );
        assert!(
            build
                .build
                .commands
                .iter()
                .any(|c| c.contains("npm install") || c.contains("npm ci")),
            "Should have npm install or npm ci command"
        );
        assert!(
            build
                .build
                .commands
                .iter()
                .any(|c| c.contains("npm run build")),
            "Should have npm run build command"
        );

        // Build packages should include both PHP and Node.js packages
        assert!(
            build.build.packages.iter().any(|p| p.starts_with("php")),
            "Should have PHP build packages"
        );
        assert!(
            build.build.packages.iter().any(|p| p.starts_with("nodejs")),
            "Should have Node.js build packages"
        );

        // Build cache should include both .composer/cache and .npm
        assert!(
            build.build.cache.contains(&".composer/cache".to_string()),
            "Should have composer cache"
        );
        assert!(
            build.build.cache.contains(&".npm".to_string()),
            "Should have npm cache"
        );

        // Runtime should be PHP-only (no Node.js packages)
        assert!(
            !build
                .runtime
                .packages
                .iter()
                .any(|p| p.starts_with("nodejs")),
            "Runtime should not have Node.js packages"
        );
        assert_eq!(build.runtime.ports, vec![8000]);
    }
}
