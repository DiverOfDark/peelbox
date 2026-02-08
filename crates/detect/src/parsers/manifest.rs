//! Manifest file parsers.
//!
//! Each parser reads a specific manifest format and normalizes it into a generic `Manifest`.

use crate::traits::ManifestParser;
use crate::types::*;
use peelbox_stack::{BuildSystemId, LanguageId, RuntimeId};
use std::collections::BTreeMap;
use std::path::Path;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn btree(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn extract_toml_string_array(val: &toml::Value, key: &str) -> Vec<String> {
    val.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

// ── CargoTomlParser ─────────────────────────────────────────────────────────

pub struct CargoTomlParser;

impl ManifestParser for CargoTomlParser {
    fn filenames(&self) -> &[&str] {
        &["Cargo.toml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let toml_val: toml::Value = toml::from_str(content).ok()?;

        let package = toml_val.get("package").and_then(|pkg| {
            let name = pkg.get("name")?.as_str()?.to_string();
            let version = pkg
                .get("version")
                .and_then(|v| v.as_str())
                .map(String::from);
            Some(Package {
                name,
                version,
                is_application: true,
            })
        });

        let workspace = toml_val.get("workspace").map(|ws| Workspace {
            members: extract_toml_string_array(ws, "members"),
            orchestrator: None,
        });

        if package.is_none() && workspace.is_none() {
            return None;
        }

        let bin_name = package
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "app".to_string());

        let dependencies = parse_cargo_deps(&toml_val);

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Rust,
            build_system: BuildSystemId::Cargo,
            runtime: RuntimeId::Native,
            package,
            workspace,
            dependencies,
            build: BuildSpec {
                packages: vec![
                    "rust".into(),
                    "build-base".into(),
                    "openssl-dev".into(),
                    "pkgconf".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["cargo build --release".into()],
                member_transform: Some(MemberBuildTransform {
                    member_commands: vec![
                        "cargo build --release --manifest-path {module}/Cargo.toml --target-dir target".into(),
                    ],
                    member_artifacts: Some(vec![(
                        format!("target/release/{}", bin_name),
                        format!("/app/{}", bin_name),
                    )]),
                }),
                env: btree(&[("CARGO_HOME", ".cargo")]),
                cache_dirs: vec![".cargo".into(), "target".into()],
                artifacts: vec![(
                    format!("target/release/{}", bin_name),
                    format!("/app/{}", bin_name),
                )],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", bin_name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

fn parse_cargo_deps(toml_val: &toml::Value) -> Vec<Dependency> {
    let mut deps = Vec::new();
    for (section, scope) in &[
        ("dependencies", DepScope::Runtime),
        ("dev-dependencies", DepScope::Dev),
        ("build-dependencies", DepScope::Build),
    ] {
        if let Some(table) = toml_val.get(section).and_then(|v| v.as_table()) {
            for (name, val) in table {
                let version = match val {
                    toml::Value::String(s) => Some(s.clone()),
                    toml::Value::Table(t) => t
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    _ => None,
                };
                deps.push(Dependency {
                    name: name.clone(),
                    version,
                    scope: scope.clone(),
                    is_internal: false,
                });
            }
        }
    }
    deps
}

// ── PackageJsonParser ───────────────────────────────────────────────────────

pub struct PackageJsonParser;

impl ManifestParser for PackageJsonParser {
    fn filenames(&self) -> &[&str] {
        &["package.json"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let json: serde_json::Value = serde_json::from_str(content).ok()?;

        let name = json.get("name").and_then(|v| v.as_str()).map(String::from);
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from);
        let has_start = json
            .get("scripts")
            .and_then(|s| s.get("start"))
            .is_some();

        let build_system = match json.get("packageManager").and_then(|v| v.as_str()) {
            Some(pm) if pm.starts_with("yarn") => BuildSystemId::Yarn,
            Some(pm) if pm.starts_with("pnpm") => BuildSystemId::Pnpm,
            Some(pm) if pm.starts_with("bun") => BuildSystemId::Bun,
            _ => BuildSystemId::Npm,
        };

        let pkg_manager = match &build_system {
            BuildSystemId::Yarn => "yarn",
            BuildSystemId::Pnpm => "pnpm",
            BuildSystemId::Bun => "bun",
            _ => "npm",
        };

        let workspace = json.get("workspaces").and_then(|ws| {
            let members = match ws {
                serde_json::Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                serde_json::Value::Object(obj) => obj
                    .get("packages")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                _ => return None,
            };
            Some(Workspace {
                members,
                orchestrator: None,
            })
        });

        let dependencies = parse_npm_deps(&json);

        // Only detect TypeScript if it's a runtime dependency (not devDependency)
        let language = if dependencies
            .iter()
            .any(|d| d.name == "typescript" && d.scope == DepScope::Runtime)
        {
            LanguageId::TypeScript
        } else {
            LanguageId::JavaScript
        };

        let entrypoint = json
            .get("main")
            .and_then(|v| v.as_str())
            .map(|m| format!("node {}", m));

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system,
            runtime: RuntimeId::Node,
            package: name.as_ref().map(|n| Package {
                name: n.clone(),
                version,
                is_application: has_start,
            }),
            workspace,
            dependencies,
            build: BuildSpec {
                packages: vec!["nodejs".into(), pkg_manager.into(), "ca-certificates".into()],
                commands: vec![
                    "npm ci".to_string(),
                    format!("{} run build", pkg_manager),
                ],
                member_transform: Some(MemberBuildTransform {
                    member_commands: vec![
                        "npm ci".to_string(),
                        format!("cd {{module}} && {} run build", pkg_manager),
                    ],
                    member_artifacts: None,
                }),
                env: BTreeMap::new(),
                cache_dirs: vec![".npm".into(), "node_modules".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "nodejs".into(),
                    "npm".into(),
                    "busybox".into(),
                    "dumb-init".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![3000],
                health_endpoint: None,
            },
        })
    }
}

fn parse_npm_deps(json: &serde_json::Value) -> Vec<Dependency> {
    let mut deps = Vec::new();
    for (section, scope) in &[
        ("dependencies", DepScope::Runtime),
        ("devDependencies", DepScope::Dev),
        ("peerDependencies", DepScope::Peer),
    ] {
        if let Some(obj) = json.get(section).and_then(|v| v.as_object()) {
            for (name, ver) in obj {
                deps.push(Dependency {
                    name: name.clone(),
                    version: ver.as_str().map(String::from),
                    scope: scope.clone(),
                    is_internal: false,
                });
            }
        }
    }
    deps
}

// ── YarnLockParser ──────────────────────────────────────────────────────────

/// Detects Yarn projects by presence of yarn.lock.
/// Reads the sibling package.json for project info but overrides build system to Yarn.
pub struct YarnLockParser;

impl ManifestParser for YarnLockParser {
    fn filenames(&self) -> &[&str] {
        &["yarn.lock"]
    }

    fn parse(&self, path: &Path, _content: &str) -> Option<Manifest> {
        // Read the sibling package.json
        let dir = path.parent()?;
        let pkg_json_path = dir.join("package.json");
        // Use the relative path for the sibling
        let abs_pkg_json = if pkg_json_path.is_absolute() {
            pkg_json_path
        } else {
            // During pipeline, the path is relative, we need to find it
            // The PackageJsonParser will be called separately; this parser
            // needs to read from absolute path resolved during tree walk.
            // We return None if we can't find it.
            return None;
        };
        let pkg_content = std::fs::read_to_string(&abs_pkg_json).ok()?;
        let json: serde_json::Value = serde_json::from_str(&pkg_content).ok()?;

        let name = json.get("name").and_then(|v| v.as_str()).map(String::from);
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from);
        let has_start = json
            .get("scripts")
            .and_then(|s| s.get("start"))
            .is_some();

        let dependencies = parse_npm_deps(&json);

        let language = if dependencies
            .iter()
            .any(|d| d.name == "typescript" && d.scope == DepScope::Runtime)
        {
            LanguageId::TypeScript
        } else {
            LanguageId::JavaScript
        };

        let entrypoint = json
            .get("main")
            .and_then(|v| v.as_str())
            .map(|m| format!("node {}", m));

        // Detect tsc build script
        let has_tsc = json
            .get("scripts")
            .and_then(|s| s.get("build"))
            .and_then(|v| v.as_str())
            .map(|s| s.contains("tsc"))
            .unwrap_or(false);

        let build_cmd = if has_tsc {
            "./node_modules/.bin/tsc".to_string()
        } else {
            "yarn run build".to_string()
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: BuildSystemId::Yarn,
            runtime: RuntimeId::Node,
            package: name.as_ref().map(|n| Package {
                name: n.clone(),
                version,
                is_application: has_start,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![
                    "nodejs".into(),
                    "yarn".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "yarn install --network-timeout 100000 --network-concurrency 1".into(),
                    build_cmd,
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".yarn-cache".into(), "node_modules".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "nodejs".into(),
                    "npm".into(),
                    "busybox".into(),
                    "dumb-init".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![3000],
                health_endpoint: None,
            },
        })
    }
}

// ── PnpmLockParser ──────────────────────────────────────────────────────────

/// Detects pnpm projects by presence of pnpm-lock.yaml.
pub struct PnpmLockParser;

impl ManifestParser for PnpmLockParser {
    fn filenames(&self) -> &[&str] {
        &["pnpm-lock.yaml"]
    }

    fn parse(&self, path: &Path, _content: &str) -> Option<Manifest> {
        let dir = path.parent()?;
        let pkg_json_path = dir.join("package.json");
        let abs_pkg_json = if pkg_json_path.is_absolute() {
            pkg_json_path
        } else {
            return None;
        };
        let pkg_content = std::fs::read_to_string(&abs_pkg_json).ok()?;
        let json: serde_json::Value = serde_json::from_str(&pkg_content).ok()?;

        let name = json.get("name").and_then(|v| v.as_str()).map(String::from);
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from);
        let has_start = json
            .get("scripts")
            .and_then(|s| s.get("start"))
            .is_some();

        let dependencies = parse_npm_deps(&json);

        let language = if dependencies
            .iter()
            .any(|d| d.name == "typescript" && d.scope == DepScope::Runtime)
        {
            LanguageId::TypeScript
        } else {
            LanguageId::JavaScript
        };

        let entrypoint = json
            .get("main")
            .and_then(|v| v.as_str())
            .map(|m| format!("node {}", m));

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: BuildSystemId::Pnpm,
            runtime: RuntimeId::Node,
            package: name.as_ref().map(|n| Package {
                name: n.clone(),
                version,
                is_application: has_start,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![
                    "nodejs".into(),
                    "pnpm".into(),
                    "build-base".into(),
                    "python".into(),
                    "npm".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["pnpm install".into(), "pnpm build".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".pnpm-store".into(), "node_modules".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "nodejs".into(),
                    "npm".into(),
                    "busybox".into(),
                    "dumb-init".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![3000],
                health_endpoint: None,
            },
        })
    }
}

// ── PomXmlParser ────────────────────────────────────────────────────────────

pub struct PomXmlParser;

impl ManifestParser for PomXmlParser {
    fn filenames(&self) -> &[&str] {
        &["pom.xml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("<project") && !content.contains("<artifactId>") {
            return None;
        }

        let doc = roxmltree::Document::parse(content).ok()?;
        let root = doc.root_element();

        let mut artifact_id = None;
        let mut version = None;
        let mut packaging = None;
        let mut parent_version = None;
        let mut modules = Vec::new();

        for child in root.children() {
            if child.has_tag_name("artifactId") && artifact_id.is_none() {
                artifact_id = child.text().map(|s| s.trim().to_string());
            }
            if child.has_tag_name("version") {
                version = child.text().map(|s| s.trim().to_string());
            }
            if child.has_tag_name("packaging") {
                packaging = child.text().map(|s| s.trim().to_string());
            }
            if child.has_tag_name("parent") {
                for pc in child.children() {
                    if pc.has_tag_name("version") {
                        parent_version = pc.text().map(|s| s.trim().to_string());
                    }
                }
            }
            if child.has_tag_name("modules") {
                for mc in child.children() {
                    if mc.has_tag_name("module") {
                        if let Some(text) = mc.text() {
                            modules.push(text.trim().to_string());
                        }
                    }
                }
            }
        }

        let name = artifact_id?;
        let effective_version = version.or(parent_version);
        let is_pom = packaging.as_deref() == Some("pom");
        let is_application = !is_pom;

        let workspace = if !modules.is_empty() {
            Some(Workspace {
                members: modules,
                orchestrator: None,
            })
        } else {
            None
        };

        let dependencies = parse_maven_deps(&doc);

        let java_version = detect_java_version_from_pom(content);

        let has_spring_boot_plugin = content.contains("spring-boot-maven-plugin");

        let entrypoint = if has_spring_boot_plugin {
            let jar_name = match &effective_version {
                Some(ver) => format!("/app/{}-{}.jar", name, ver),
                None => format!("/app/{}.jar", name),
            };
            Some(format!("java -jar {}", jar_name))
        } else {
            extract_main_class(&doc).map(|mc| format!("java -cp /app/*.jar {}", mc))
        };

        let java_pkg = java_version
            .as_ref()
            .map(|v| format!("openjdk-{}", v))
            .unwrap_or_else(|| "openjdk".into());

        let target_dir = "target".to_string();

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Java,
            build_system: BuildSystemId::Maven,
            runtime: RuntimeId::JVM,
            package: Some(Package {
                name: name.clone(),
                version: effective_version,
                is_application,
            }),
            workspace,
            dependencies,
            build: BuildSpec {
                packages: vec![java_pkg.clone(), "maven".into(), "ca-certificates".into()],
                commands: vec![
                    "mvn package -DskipTests".into(),
                    format!(
                        "mvn dependency:copy-dependencies -DoutputDirectory={}/lib",
                        target_dir
                    ),
                ],
                member_transform: Some(MemberBuildTransform {
                    member_commands: vec![
                        "mvn -pl {module} -am install -DskipTests".into(),
                        "mvn -pl {module} dependency:copy-dependencies -DoutputDirectory=target/lib"
                            .into(),
                    ],
                    member_artifacts: Some(vec![
                        ("{module}/target/*.jar".into(), "/app/".into()),
                        ("{module}/target/lib/".into(), "/app/lib".into()),
                    ]),
                }),
                env: btree(&[
                    (
                        "JAVA_HOME",
                        &format!(
                            "/usr/lib/jvm/java-{}-openjdk",
                            java_version.as_deref().unwrap_or("21")
                        ),
                    ),
                    ("MAVEN_OPTS", "-Dmaven.repo.local=/root/.m2/repository"),
                ]),
                cache_dirs: vec!["/root/.m2/repository".into()],
                artifacts: vec![
                    (format!("{}/*.jar", target_dir), "/app/".into()),
                    (format!("{}/lib/", target_dir), "/app/lib".into()),
                ],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    java_version
                        .as_ref()
                        .map(|v| format!("openjdk-{}-jre", v))
                        .unwrap_or_else(|| "openjdk".into()),
                    "ca-certificates".into(),
                ],
                env: btree(&[("CLASSPATH", "/app/*:/app/lib/*")]),
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

fn parse_maven_deps(doc: &roxmltree::Document) -> Vec<Dependency> {
    let mut deps = Vec::new();
    for node in doc.descendants() {
        if node.has_tag_name("dependency") {
            let mut group_id = None;
            let mut artifact_id = None;
            let mut ver = None;
            let mut scope_str = None;

            for child in node.children() {
                if child.has_tag_name("groupId") {
                    group_id = child.text().map(|s| s.trim().to_string());
                }
                if child.has_tag_name("artifactId") {
                    artifact_id = child.text().map(|s| s.trim().to_string());
                }
                if child.has_tag_name("version") {
                    ver = child.text().map(|s| s.trim().to_string());
                }
                if child.has_tag_name("scope") {
                    scope_str = child.text().map(|s| s.trim().to_string());
                }
            }

            if let (Some(gid), Some(aid)) = (group_id, artifact_id) {
                let scope = match scope_str.as_deref() {
                    Some("test") => DepScope::Dev,
                    Some("provided") => DepScope::Build,
                    _ => DepScope::Runtime,
                };
                deps.push(Dependency {
                    name: format!("{}:{}", gid, aid),
                    version: ver,
                    scope,
                    is_internal: false,
                });
            }
        }
    }
    deps
}

fn detect_java_version_from_pom(content: &str) -> Option<String> {
    peelbox_stack::version::java::detect_java_version(content)
}

fn extract_main_class(doc: &roxmltree::Document) -> Option<String> {
    for node in doc.descendants() {
        if node.has_tag_name("mainClass") {
            return node.text().map(|s| s.trim().to_string());
        }
    }
    None
}

// ── BuildGradleParser ───────────────────────────────────────────────────────

pub struct BuildGradleParser;

impl ManifestParser for BuildGradleParser {
    fn filenames(&self) -> &[&str] {
        &["build.gradle", "build.gradle.kts"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("plugins") && !content.contains("dependencies") {
            return None;
        }

        let java_version = peelbox_stack::version::java::detect_java_version(content);
        let java_pkg = java_version
            .as_ref()
            .map(|v| format!("openjdk-{}", v))
            .unwrap_or_else(|| "openjdk".into());

        let dependencies = parse_gradle_deps(content);

        // Try to extract version from build.gradle for artifact naming
        let _project_version = regex::Regex::new(r#"version\s*=?\s*['"]([^'"]+)['"]"#)
            .ok()
            .and_then(|re| {
                re.captures(content)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string())
            });

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Java,
            build_system: BuildSystemId::Gradle,
            runtime: RuntimeId::JVM,
            package: None, // Gradle project names come from settings.gradle
            workspace: None, // Workspace comes from SettingsGradleParser
            dependencies,
            build: BuildSpec {
                packages: vec![java_pkg.clone(), "gradle".into(), "ca-certificates".into()],
                commands: vec![
                    "gradle assemble -x test --no-daemon --console=plain".into(),
                ],
                member_transform: Some(MemberBuildTransform {
                    member_commands: vec![
                        "gradle :{module}:assemble -x test --no-daemon --console=plain".into(),
                    ],
                    member_artifacts: Some(vec![(
                        "{module}/build/libs/*.jar".into(),
                        "/app/app.jar".into(),
                    )]),
                }),
                env: btree(&[
                    (
                        "JAVA_HOME",
                        &format!(
                            "/usr/lib/jvm/java-{}-openjdk",
                            java_version.as_deref().unwrap_or("21")
                        ),
                    ),
                    ("GRADLE_USER_HOME", "/root/.gradle"),
                    ("GRADLE_OPTS", "-Dorg.gradle.native=false"),
                ]),
                cache_dirs: vec![".gradle".into(), "build".into()],
                artifacts: vec![("build/libs/*.jar".into(), "/app/app.jar".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    java_version
                        .as_ref()
                        .map(|v| format!("openjdk-{}-jre", v))
                        .unwrap_or_else(|| "openjdk".into()),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint: Some("java -jar /app/app.jar".into()),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

fn parse_gradle_deps(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let dep_re =
        regex::Regex::new(r#"(?:implementation|api|compile|runtimeOnly|testImplementation)\s*[\("]([^")\s]+)"#)
            .unwrap();
    for cap in dep_re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            let dep_str = m.as_str();
            let parts: Vec<&str> = dep_str.splitn(3, ':').collect();
            if parts.len() >= 2 {
                let scope = if cap
                    .get(0)
                    .map(|m| m.as_str().contains("test"))
                    .unwrap_or(false)
                {
                    DepScope::Dev
                } else {
                    DepScope::Runtime
                };
                deps.push(Dependency {
                    name: format!("{}:{}", parts[0], parts[1]),
                    version: parts.get(2).map(|s| s.to_string()),
                    scope,
                    is_internal: false,
                });
            }
        }
    }
    deps
}

// ── SettingsGradleParser ────────────────────────────────────────────────────

pub struct SettingsGradleParser;

impl ManifestParser for SettingsGradleParser {
    fn filenames(&self) -> &[&str] {
        &["settings.gradle", "settings.gradle.kts"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let mut members = Vec::new();
        let mut project_name = None;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.contains("rootProject.name") {
                if let Some(value) = trimmed.split('=').nth(1) {
                    let name = value
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                    if !name.is_empty() {
                        project_name = Some(name);
                    }
                }
            }

            if trimmed.starts_with("include") {
                if let Some(projects_str) = trimmed
                    .split('(')
                    .nth(1)
                    .and_then(|s| s.split(')').next())
                {
                    for project in projects_str.split(',') {
                        let project = project.trim().trim_matches(|c| c == '\'' || c == '"');
                        if !project.is_empty() {
                            members.push(project.trim_start_matches(':').to_string());
                        }
                    }
                }
            }
        }

        if members.is_empty() && project_name.is_none() {
            return None;
        }

        let workspace = if !members.is_empty() {
            Some(Workspace {
                members,
                orchestrator: None,
            })
        } else {
            None
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Java,
            build_system: BuildSystemId::Gradle,
            runtime: RuntimeId::JVM,
            package: project_name.map(|name| Package {
                name,
                version: None,
                is_application: true,
            }),
            workspace,
            dependencies: Vec::new(),
            build: BuildSpec::default(),
            runtime_config: RuntimeSpec::default(),
        })
    }
}

// ── GoModParser ─────────────────────────────────────────────────────────────

pub struct GoModParser;

impl ManifestParser for GoModParser {
    fn filenames(&self) -> &[&str] {
        &["go.mod"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let module_name = content
            .lines()
            .find(|l| l.starts_with("module "))
            .map(|l| l.trim_start_matches("module ").trim().to_string())?;

        let short_name = module_name.rsplit('/').next().unwrap_or(&module_name);

        // Extract Go version from go.mod (e.g., "go 1.21")
        let go_version = content
            .lines()
            .find(|l| l.starts_with("go "))
            .and_then(|l| {
                let ver = l.trim_start_matches("go ").trim();
                if ver.is_empty() {
                    None
                } else {
                    Some(ver.to_string())
                }
            });

        let go_pkg = go_version
            .as_ref()
            .map(|v| format!("go-{}", v))
            .unwrap_or_else(|| "go".into());

        let dependencies = parse_go_deps(content);

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Go,
            build_system: BuildSystemId::GoMod,
            runtime: RuntimeId::Native,
            package: Some(Package {
                name: short_name.to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![go_pkg, "ca-certificates".into()],
                commands: vec![
                    "go mod download".into(),
                    format!("go build -o {} .", short_name),
                ],
                member_transform: None,
                env: btree(&[
                    ("CGO_ENABLED", "0"),
                    ("GOCACHE", "/build/.cache/go-build"),
                    ("GOMODCACHE", "/build/.cache/go-mod"),
                    ("GOSUMDB", "off"),
                ]),
                cache_dirs: vec![".cache/go-build".into(), ".cache/go-mod".into()],
                artifacts: vec![(short_name.to_string(), format!("/app/{}", short_name))],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", short_name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

fn parse_go_deps(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let mut in_require = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "require (" {
            in_require = true;
            continue;
        }
        if trimmed == ")" {
            in_require = false;
            continue;
        }
        // Handle single-line require
        if trimmed.starts_with("require ") && !trimmed.ends_with("(") {
            let rest = trimmed.strip_prefix("require ").unwrap_or("");
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                deps.push(Dependency {
                    name: parts[0].to_string(),
                    version: Some(parts[1].to_string()),
                    scope: DepScope::Runtime,
                    is_internal: false,
                });
            }
            continue;
        }
        if in_require && !trimmed.starts_with("//") {
            let clean = trimmed.split("//").next().unwrap_or(trimmed).trim();
            let parts: Vec<&str> = clean.split_whitespace().collect();
            if parts.len() >= 2 {
                deps.push(Dependency {
                    name: parts[0].to_string(),
                    version: Some(parts[1].to_string()),
                    scope: DepScope::Runtime,
                    is_internal: false,
                });
            }
        }
    }
    deps
}

// ── PyProjectTomlParser ─────────────────────────────────────────────────────

pub struct PyProjectTomlParser;

impl ManifestParser for PyProjectTomlParser {
    fn filenames(&self) -> &[&str] {
        &["pyproject.toml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let toml_val: toml::Value = toml::from_str(content).ok()?;

        let is_poetry = toml_val
            .get("tool")
            .and_then(|t| t.get("poetry"))
            .is_some();

        let (name, version) = if is_poetry {
            let poetry = toml_val.get("tool").and_then(|t| t.get("poetry"));
            let name = poetry
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let version = poetry
                .and_then(|p| p.get("version"))
                .and_then(|v| v.as_str())
                .map(String::from);
            (name, version)
        } else {
            let project = toml_val.get("project");
            let name = project
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let version = project
                .and_then(|p| p.get("version"))
                .and_then(|v| v.as_str())
                .map(String::from);
            (name, version)
        };

        let build_system_id = if is_poetry {
            BuildSystemId::Poetry
        } else {
            BuildSystemId::Pip
        };

        let dependencies = parse_pyproject_deps(&toml_val, is_poetry);

        if is_poetry {
            // Poetry-specific build
            Some(Manifest {
                path: path.to_path_buf(),
                language: LanguageId::Python,
                build_system: build_system_id,
                runtime: RuntimeId::Python,
                package: Some(Package {
                    name: name.unwrap_or_else(|| "app".to_string()),
                    version,
                    is_application: true,
                }),
                workspace: None,
                dependencies,
                build: BuildSpec {
                    packages: vec![
                        "python".into(),
                        "pip".into(),
                        "build-base".into(),
                        "ca-certificates".into(),
                    ],
                    commands: vec![
                        "pip install --user poetry".into(),
                        "/root/.local/bin/poetry install --no-root --only main".into(),
                    ],
                    member_transform: None,
                    env: btree(&[
                        ("POETRY_CACHE_DIR", "/root/.cache/pypoetry"),
                        ("POETRY_VIRTUALENVS_IN_PROJECT", "true"),
                    ]),
                    cache_dirs: vec![
                        "/root/.cache/pip/".into(),
                        "/root/.cache/pypoetry/".into(),
                    ],
                    artifacts: vec![(".".into(), "/build".into())],
                },
                runtime_config: RuntimeSpec {
                    packages: vec![
                        "python".into(),
                        "libgcc".into(),
                        "libstdc++".into(),
                        "ca-certificates".into(),
                    ],
                    env: BTreeMap::new(),
                    entrypoint: None, // Will be set by framework detector (Flask)
                    workdir: Some("/build".into()),
                    ports: vec![8000],
                    health_endpoint: None,
                },
            })
        } else {
            Some(Manifest {
                path: path.to_path_buf(),
                language: LanguageId::Python,
                build_system: build_system_id,
                runtime: RuntimeId::Python,
                package: name.as_ref().map(|n| Package {
                    name: n.clone(),
                    version,
                    is_application: true,
                }),
                workspace: None,
                dependencies,
                build: BuildSpec {
                    packages: vec![
                        "python".into(),
                        "pip".into(),
                        "build-base".into(),
                        "ca-certificates".into(),
                    ],
                    commands: vec!["pip install --user --no-cache-dir .".into()],
                    member_transform: None,
                    env: BTreeMap::new(),
                    cache_dirs: vec![".cache/pip".into()],
                    artifacts: vec![(".".into(), "/app/".into())],
                },
                runtime_config: RuntimeSpec {
                    packages: vec![
                        "python".into(),
                        "libgcc".into(),
                        "libstdc++".into(),
                        "ca-certificates".into(),
                    ],
                    env: BTreeMap::new(),
                    entrypoint: name
                        .as_ref()
                        .map(|n| format!("python -m {}", n.replace('-', "_"))),
                    workdir: Some("/app".into()),
                    ports: vec![8000],
                    health_endpoint: None,
                },
            })
        }
    }
}

fn parse_pyproject_deps(toml_val: &toml::Value, is_poetry: bool) -> Vec<Dependency> {
    let mut deps = Vec::new();

    if is_poetry {
        if let Some(poetry_deps) = toml_val
            .get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("dependencies"))
            .and_then(|d| d.as_table())
        {
            for (name, val) in poetry_deps {
                if name == "python" {
                    continue;
                }
                let version = match val {
                    toml::Value::String(s) => Some(s.clone()),
                    toml::Value::Table(t) => t
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    _ => None,
                };
                deps.push(Dependency {
                    name: name.clone(),
                    version,
                    scope: DepScope::Runtime,
                    is_internal: false,
                });
            }
        }
    } else if let Some(dep_list) = toml_val
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
    {
        for dep in dep_list {
            if let Some(dep_str) = dep.as_str() {
                let name = dep_str
                    .split(&['>', '<', '=', '~', '!', '['][..])
                    .next()
                    .unwrap_or(dep_str)
                    .trim()
                    .to_string();
                deps.push(Dependency {
                    name,
                    version: None,
                    scope: DepScope::Runtime,
                    is_internal: false,
                });
            }
        }
    }

    deps
}

// ── RequirementsTxtParser ───────────────────────────────────────────────────

pub struct RequirementsTxtParser;

impl ManifestParser for RequirementsTxtParser {
    fn filenames(&self) -> &[&str] {
        &["requirements.txt"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let deps: Vec<Dependency> = content
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with('-'))
            .map(|l| {
                let name = l
                    .split(&['>', '<', '=', '~', '!', '['][..])
                    .next()
                    .unwrap_or(l)
                    .trim()
                    .to_string();
                Dependency {
                    name,
                    version: None,
                    scope: DepScope::Runtime,
                    is_internal: false,
                }
            })
            .collect();

        if deps.is_empty() {
            return None;
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Python,
            build_system: BuildSystemId::Pip,
            runtime: RuntimeId::Python,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: deps,
            build: BuildSpec {
                packages: vec![
                    "python".into(),
                    "pip".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "pip install --user --no-cache-dir -r requirements.txt".into(),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".cache/pip".into()],
                artifacts: vec![(".".into(), "/app/".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "python".into(),
                    "libgcc".into(),
                    "libstdc++".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint: None,
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

// ── SetupPyParser ───────────────────────────────────────────────────────────

pub struct SetupPyParser;

impl ManifestParser for SetupPyParser {
    fn filenames(&self) -> &[&str] {
        &["setup.py"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("setup(") {
            return None;
        }

        let name_re = regex::Regex::new(r#"name\s*=\s*['"]([^'"]+)['"]"#).ok()?;
        let name = name_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Python,
            build_system: BuildSystemId::Pip,
            runtime: RuntimeId::Python,
            package: name.as_ref().map(|n| Package {
                name: n.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "python".into(),
                    "pip".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["pip install --user --no-cache-dir .".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".cache/pip".into()],
                artifacts: vec![(".".into(), "/app/".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "python".into(),
                    "libgcc".into(),
                    "libstdc++".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint: name.map(|n| format!("python -m {}", n.replace('-', "_"))),
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

// ── GemfileParser ───────────────────────────────────────────────────────────

pub struct GemfileParser;

impl ManifestParser for GemfileParser {
    fn filenames(&self) -> &[&str] {
        &["Gemfile"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("gem ") && !content.contains("source ") {
            return None;
        }

        let deps: Vec<Dependency> = content
            .lines()
            .filter_map(|l| {
                let trimmed = l.trim();
                if trimmed.starts_with("gem ") {
                    let name = trimmed
                        .trim_start_matches("gem ")
                        .split(',')
                        .next()?
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string();
                    Some(Dependency {
                        name,
                        version: None,
                        scope: DepScope::Runtime,
                        is_internal: false,
                    })
                } else {
                    None
                }
            })
            .collect();

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Ruby,
            build_system: BuildSystemId::Bundler,
            runtime: RuntimeId::Ruby,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: deps,
            build: BuildSpec {
                packages: vec![
                    "ruby".into(),
                    "ruby-dev".into(),
                    "ruby-bundler".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["bundle install".into()],
                member_transform: None,
                env: btree(&[
                    ("BUNDLE_DEPLOYMENT", "false"),
                    ("BUNDLE_PATH", "vendor/bundle"),
                ]),
                cache_dirs: vec![".bundle".into(), "vendor".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "ruby".into(),
                    "ruby-bundler".into(),
                    "libgcc".into(),
                    "libstdc++".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint: None, // Set by framework detector (Sinatra/Rails)
                workdir: Some("/app".into()),
                ports: vec![3000],
                health_endpoint: None,
            },
        })
    }
}

// ── CsprojParser ────────────────────────────────────────────────────────────

pub struct CsprojParser;

impl ManifestParser for CsprojParser {
    fn filenames(&self) -> &[&str] {
        // Matched by extension in pipeline.rs classify_file
        &[]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("<Project") {
            return None;
        }

        let is_fsharp = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "fsproj")
            .unwrap_or(false);

        let language = if is_fsharp {
            LanguageId::FSharp
        } else {
            LanguageId::CSharp
        };

        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(String::from);

        // Extract .NET version from TargetFramework (e.g., "net8.0" -> "8")
        let dotnet_version =
            regex::Regex::new(r"<TargetFramework>net(\d+)\.\d+</TargetFramework>")
                .ok()
                .and_then(|re| {
                    re.captures(content)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                });

        let sdk_pkg = dotnet_version
            .as_ref()
            .map(|v| format!("dotnet-{}-sdk", v))
            .unwrap_or_else(|| "dotnet-sdk".into());

        let runtime_pkg = dotnet_version
            .as_ref()
            .map(|v| format!("aspnet-{}-runtime", v))
            .unwrap_or_else(|| "dotnet-runtime".into());

        // Parse dependencies from PackageReference elements
        let dependencies = parse_csproj_deps(content);

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: BuildSystemId::DotNet,
            runtime: RuntimeId::DotNet,
            package: Some(Package {
                name: "app".to_string(), // Normalized to "app"
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![sdk_pkg, "ca-certificates".into()],
                commands: vec![
                    "dotnet restore".into(),
                    "dotnet publish -c Release -o out".into(),
                ],
                member_transform: None,
                env: btree(&[
                    ("DOTNET_CLI_HOME", "/root"),
                    ("DOTNET_CLI_TELEMETRY_OPTOUT", "1"),
                    ("DOTNET_NOLOGO", "1"),
                    ("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1"),
                ]),
                cache_dirs: vec![".nuget/packages".into(), "bin".into(), "obj".into()],
                artifacts: vec![("out/".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![runtime_pkg, "ca-certificates".into()],
                env: BTreeMap::new(), // ASPNETCORE_URLS set by framework detector
                entrypoint: file_stem.map(|n| format!("dotnet /app/{}.dll", n)),
                workdir: Some("/app".into()),
                ports: vec![5000],
                health_endpoint: None,
            },
        })
    }
}

fn parse_csproj_deps(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let re = regex::Regex::new(r#"<PackageReference\s+Include="([^"]+)""#).unwrap();
    for cap in re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            deps.push(Dependency {
                name: m.as_str().to_string(),
                version: None,
                scope: DepScope::Runtime,
                is_internal: false,
            });
        }
    }
    deps
}

// ── MixExsParser ────────────────────────────────────────────────────────────

pub struct MixExsParser;

impl ManifestParser for MixExsParser {
    fn filenames(&self) -> &[&str] {
        &["mix.exs"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("defmodule") || !content.contains("def project") {
            return None;
        }

        let app_re = regex::Regex::new(r#"app:\s*:(\w+)"#).ok()?;
        let name = app_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Elixir,
            build_system: BuildSystemId::Mix,
            runtime: RuntimeId::BEAM,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "elixir".into(),
                    "erlang".into(),
                    "erlang-dev".into(),
                    "git".into(),
                    "build-base".into(),
                    "openssl".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "mix local.hex --force && mix local.rebar --force".into(),
                    "mix deps.get".into(),
                    "mix compile".into(),
                ],
                member_transform: None,
                env: btree(&[
                    ("ELIXIR_ERL_OPTIONS", "+fnu"),
                    ("LC_ALL", "C.UTF-8"),
                    ("MIX_ENV", "prod"),
                    ("MIX_HOME", "/build/.mix"),
                ]),
                cache_dirs: vec!["deps".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "elixir".into(),
                    "erlang".into(),
                    "busybox".into(),
                    "openssl".into(),
                    "ca-certificates".into(),
                ],
                env: btree(&[
                    ("ELIXIR_ERL_OPTIONS", "+fnu"),
                    ("LC_ALL", "C.UTF-8"),
                    ("MIX_ENV", "prod"),
                    ("MIX_HOME", "/app/.mix"),
                    ("PORT", "4000"),
                ]),
                entrypoint: Some("mix run --no-halt".into()),
                workdir: Some("/app".into()),
                ports: vec![4000],
                health_endpoint: Some("/health".into()),
            },
        })
    }
}

// ── ComposerJsonParser ──────────────────────────────────────────────────────

pub struct ComposerJsonParser;

impl ManifestParser for ComposerJsonParser {
    fn filenames(&self) -> &[&str] {
        &["composer.json"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let json: serde_json::Value = serde_json::from_str(content).ok()?;

        // Extract PHP version requirement (e.g., ">=8.1" -> "8.1")
        let php_version = json
            .get("require")
            .and_then(|r| r.get("php"))
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let re = regex::Regex::new(r"(\d+\.\d+)").ok()?;
                re.captures(s)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string())
            });

        let php_pkg = php_version
            .as_ref()
            .map(|v| format!("php-{}", v))
            .unwrap_or_else(|| "php".into());

        let deps: Vec<Dependency> = json
            .get("require")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter(|(k, _)| *k != "php")
                    .map(|(k, v)| Dependency {
                        name: k.clone(),
                        version: v.as_str().map(String::from),
                        scope: DepScope::Runtime,
                        is_internal: false,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Detect if Symfony
        let is_symfony = deps.iter().any(|d| d.name.starts_with("symfony/"));
        let has_runtime_plugin = deps.iter().any(|d| d.name == "symfony/runtime");

        // Build PHP extensions list
        let php_extensions: Vec<String> = php_version
            .as_ref()
            .map(|v| {
                vec![
                    format!("php-{}-ctype", v),
                    format!("php-{}-phar", v),
                    format!("php-{}-openssl", v),
                    format!("php-{}-mbstring", v),
                    format!("php-{}-xml", v),
                    format!("php-{}-dom", v),
                    format!("php-{}-curl", v),
                ]
            })
            .unwrap_or_default();

        let mut build_packages = vec![
            php_pkg.clone(),
            "composer".into(),
            "unzip".into(),
            "git".into(),
            "ca-certificates".into(),
        ];
        build_packages.extend(php_extensions.clone());

        let mut build_commands = Vec::new();
        if is_symfony && has_runtime_plugin {
            build_commands.push("composer config allow-plugins.symfony/runtime true".into());
        }
        build_commands
            .push("composer install --no-dev --optimize-autoloader --ignore-platform-reqs".into());

        // Runtime PHP extensions (base + extras)
        let mut runtime_packages = vec![php_pkg.clone()];
        runtime_packages.extend(php_extensions);
        if let Some(v) = &php_version {
            runtime_packages.push(format!("php-{}-fileinfo", v));
            runtime_packages.push(format!("php-{}-iconv", v));
            if is_symfony {
                runtime_packages.push(format!("php-{}-intl", v));
                runtime_packages.push(format!("php-{}-pdo", v));
            }
        }
        runtime_packages.push("ca-certificates".into());

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::PHP,
            build_system: BuildSystemId::Composer,
            runtime: RuntimeId::PHP,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: deps,
            build: BuildSpec {
                packages: build_packages,
                commands: build_commands,
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".composer/cache".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: runtime_packages,
                env: BTreeMap::new(),
                entrypoint: Some("php -S 0.0.0.0:8000 index.php".into()),
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

// ── CMakeListsParser ────────────────────────────────────────────────────────

pub struct CMakeListsParser;

impl ManifestParser for CMakeListsParser {
    fn filenames(&self) -> &[&str] {
        &["CMakeLists.txt"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("cmake_minimum_required") && !content.contains("project(") {
            return None;
        }

        let name_re = regex::Regex::new(r"project\s*\(\s*(\w+)").ok()?;
        let name = name_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Cpp,
            build_system: BuildSystemId::CMake,
            runtime: RuntimeId::Native,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "build-base".into(),
                    "cmake".into(),
                    "gcc".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "cmake -B build -DCMAKE_BUILD_TYPE=Release".into(),
                    "cmake --build build --config Release".into(),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["CMakeCache.txt".into(), "build".into()],
                artifacts: vec![(format!("build/{}", name), format!("/app/{}", name))],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

// ── MakefileParser ──────────────────────────────────────────────────────────

pub struct MakefileParser;

impl ManifestParser for MakefileParser {
    fn filenames(&self) -> &[&str] {
        &["Makefile"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains(':') {
            return None;
        }

        let (language, runtime) = if content.contains("gcc") || content.contains("g++") {
            (LanguageId::Cpp, RuntimeId::Native)
        } else {
            (LanguageId::Cpp, RuntimeId::Native)
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: BuildSystemId::Make,
            runtime,
            package: None,
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "make".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["make".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: Vec::new(),
                artifacts: Vec::new(),
            },
            runtime_config: RuntimeSpec {
                packages: vec!["ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: None,
                workdir: Some("/app".into()),
                ports: Vec::new(),
                health_endpoint: None,
            },
        })
    }
}

// ── MesonBuildParser ────────────────────────────────────────────────────────

pub struct MesonBuildParser;

impl ManifestParser for MesonBuildParser {
    fn filenames(&self) -> &[&str] {
        &["meson.build"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("project(") {
            return None;
        }

        let name_re = regex::Regex::new(r"project\s*\(\s*'([^']+)'").ok()?;
        let name = name_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Cpp,
            build_system: BuildSystemId::Meson,
            runtime: RuntimeId::Native,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "meson".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "meson setup build --buildtype=release".into(),
                    "meson compile -C build".into(),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["build/".into()],
                artifacts: vec![(format!("build/{}", name), format!("/app/{}", name))],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

// ── BuildZigZonParser ───────────────────────────────────────────────────────

/// Parses build.zig.zon for package name and dependencies.
pub struct BuildZigZonParser;

impl ManifestParser for BuildZigZonParser {
    fn filenames(&self) -> &[&str] {
        &["build.zig.zon"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Extract .name from build.zig.zon (format: .name = .app, or .name = "app")
        let name = regex::Regex::new(r#"\.name\s*=\s*(?:\.(\w+)|"([^"]+)")"#)
            .ok()?
            .captures(content)
            .and_then(|c| {
                c.get(1)
                    .or_else(|| c.get(2))
                    .map(|m| m.as_str().to_string())
            })
            .unwrap_or_else(|| "app".to_string());

        // Parse dependencies from .dependencies section
        let mut deps = Vec::new();
        let dep_re = regex::Regex::new(r"\.(\w+)\s*=\s*\.\{").ok()?;
        let mut in_deps = false;
        let mut brace_depth = 0;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.contains(".dependencies") && trimmed.contains("= .{") {
                in_deps = true;
                brace_depth = 1;
                continue;
            }
            if in_deps {
                for ch in trimmed.chars() {
                    if ch == '{' {
                        brace_depth += 1;
                    }
                    if ch == '}' {
                        brace_depth -= 1;
                    }
                }
                if brace_depth <= 0 {
                    in_deps = false;
                    continue;
                }
                // Look for .depname = .{ patterns at depth 1
                if brace_depth == 1 {
                    if let Some(cap) = dep_re.captures(trimmed) {
                        if let Some(dep_name) = cap.get(1) {
                            deps.push(Dependency {
                                name: dep_name.as_str().to_string(),
                                version: None,
                                scope: DepScope::Runtime,
                                is_internal: false,
                            });
                        }
                    }
                }
            }
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Zig,
            build_system: BuildSystemId::Zig,
            runtime: RuntimeId::Native,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: deps,
            build: BuildSpec {
                packages: vec!["zig".into(), "build-base".into(), "ca-certificates".into()],
                commands: vec!["zig build -Doptimize=ReleaseSafe".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["zig-cache".into(), "zig-out".into()],
                artifacts: vec![
                    (format!("zig-out/bin/{}", name), format!("/app/{}", name)),
                    ("zig-out/lib/".into(), "/app/lib/".into()),
                ],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

// ── ZigBuildParser ──────────────────────────────────────────────────────────

pub struct ZigBuildParser;

impl ManifestParser for ZigBuildParser {
    fn filenames(&self) -> &[&str] {
        &["build.zig"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("std.Build") && !content.contains("@import") {
            return None;
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Zig,
            build_system: BuildSystemId::Zig,
            runtime: RuntimeId::Native,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec!["zig".into(), "build-base".into(), "ca-certificates".into()],
                commands: vec!["zig build -Doptimize=ReleaseSafe".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["zig-cache".into(), "zig-out".into()],
                artifacts: vec![("zig-out/bin/*".into(), "/app/".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some("/app/app".into()),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

// ── DenoJsonParser ──────────────────────────────────────────────────────────

pub struct DenoJsonParser;

impl ManifestParser for DenoJsonParser {
    fn filenames(&self) -> &[&str] {
        &["deno.json", "deno.jsonc"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Strip JSONC comments for parsing
        let clean = content
            .lines()
            .map(|l| {
                if let Some(idx) = l.find("//") {
                    &l[..idx]
                } else {
                    l
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let _json: serde_json::Value = serde_json::from_str(&clean).ok()?;

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Deno,
            build_system: BuildSystemId::Deno,
            runtime: RuntimeId::Deno,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec!["deno".into(), "ca-certificates".into()],
                commands: vec!["deno install".into(), "deno task build".into()],
                member_transform: None,
                env: btree(&[("DENO_DIR", "/deno-dir")]),
                cache_dirs: vec!["/deno-dir".into()],
                artifacts: vec![
                    (".".into(), "/app".into()),
                    ("/deno-dir".into(), "/deno-dir".into()),
                ],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["deno".into(), "ca-certificates".into()],
                env: btree(&[("DENO_DIR", "/deno-dir")]),
                entrypoint: Some(
                    "deno run --allow-net --allow-read --allow-env main.ts".into(),
                ),
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

// ── BazelBuildParser ────────────────────────────────────────────────────────

pub struct BazelBuildParser;

impl ManifestParser for BazelBuildParser {
    fn filenames(&self) -> &[&str] {
        &["BUILD"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Look for cc_binary, java_binary, etc.
        if !content.contains("cc_binary")
            && !content.contains("java_binary")
            && !content.contains("py_binary")
            && !content.contains("go_binary")
        {
            return None;
        }

        // Extract binary name
        let name_re = regex::Regex::new(r#"name\s*=\s*"([^"]+)""#).ok()?;
        let name = name_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        // Detect language from binary type
        let language = if content.contains("cc_binary") {
            LanguageId::Cpp
        } else if content.contains("java_binary") {
            LanguageId::Java
        } else if content.contains("py_binary") {
            LanguageId::Python
        } else {
            LanguageId::Go
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: BuildSystemId::Bazel,
            runtime: RuntimeId::Native,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "build-base".into(),
                    "bazel".into(),
                    "openjdk".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["bazel build //...".into()],
                member_transform: None,
                env: btree(&[
                    ("JAVA_HOME", "/usr/lib/jvm/java-21-openjdk"),
                    (
                        "PATH",
                        "/usr/lib/jvm/java-21-openjdk/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                    ),
                ]),
                cache_dirs: vec![".cache/bazel".into()],
                artifacts: vec![(
                    format!("bazel-bin/{}", name),
                    format!("/app/{}", name),
                )],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_toml_parser_simple() {
        let parser = CargoTomlParser;
        let content = r#"
[package]
name = "my-app"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = { version = "1.0", features = ["full"] }
"#;
        let manifest = parser.parse(Path::new("Cargo.toml"), content).unwrap();
        assert_eq!(manifest.language, LanguageId::Rust);
        assert_eq!(manifest.build_system, BuildSystemId::Cargo);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-app");
        assert_eq!(manifest.dependencies.len(), 2);
    }

    #[test]
    fn test_cargo_toml_parser_workspace() {
        let parser = CargoTomlParser;
        let content = r#"
[workspace]
members = ["crates/*", "apps/cli"]
"#;
        let manifest = parser.parse(Path::new("Cargo.toml"), content).unwrap();
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["crates/*", "apps/cli"]);
        assert!(manifest.package.is_none());
    }

    #[test]
    fn test_package_json_parser() {
        let parser = PackageJsonParser;
        let content = r#"{
            "name": "my-app",
            "version": "1.0.0",
            "scripts": { "start": "node index.js", "build": "tsc" },
            "dependencies": { "express": "^4.0.0" },
            "devDependencies": { "typescript": "^5.0.0" }
        }"#;
        let manifest = parser.parse(Path::new("package.json"), content).unwrap();
        assert_eq!(manifest.language, LanguageId::JavaScript);
        assert_eq!(manifest.build_system, BuildSystemId::Npm);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-app");
        assert!(pkg.is_application);
    }

    #[test]
    fn test_package_json_pnpm() {
        let parser = PackageJsonParser;
        let content = r#"{
            "name": "my-app",
            "packageManager": "pnpm@8.0.0",
            "dependencies": { "express": "^4.0.0" }
        }"#;
        let manifest = parser.parse(Path::new("package.json"), content).unwrap();
        assert_eq!(manifest.build_system, BuildSystemId::Pnpm);
    }

    #[test]
    fn test_pom_xml_parser() {
        let parser = PomXmlParser;
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>my-app</artifactId>
    <version>1.0.0</version>
    <properties>
        <java.version>21</java.version>
    </properties>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
    </dependencies>
</project>"#;
        let manifest = parser.parse(Path::new("pom.xml"), content).unwrap();
        assert_eq!(manifest.language, LanguageId::Java);
        assert_eq!(manifest.build_system, BuildSystemId::Maven);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-app");
        assert!(manifest.dependencies.iter().any(|d| d
            .name
            .contains("spring-boot-starter-web")));
    }

    #[test]
    fn test_go_mod_parser() {
        let parser = GoModParser;
        let content = r#"module github.com/example/myserver

go 1.21

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/lib/pq v1.10.9
)"#;
        let manifest = parser.parse(Path::new("go.mod"), content).unwrap();
        assert_eq!(manifest.language, LanguageId::Go);
        assert_eq!(manifest.build_system, BuildSystemId::GoMod);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "myserver");
        assert_eq!(manifest.dependencies.len(), 2);
    }

    #[test]
    fn test_settings_gradle_parser() {
        let parser = SettingsGradleParser;
        let content = r#"
rootProject.name = 'my-project'
include('core', 'web', 'api-service')
"#;
        let manifest = parser
            .parse(Path::new("settings.gradle"), content)
            .unwrap();
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["core", "web", "api-service"]);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-project");
    }
}
