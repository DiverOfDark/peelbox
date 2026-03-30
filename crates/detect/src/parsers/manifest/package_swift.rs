use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const SWIFT: LanguageId = LanguageId::new("swift");
const SWIFT_PM: BuildSystemId = BuildSystemId::new("swift-package-manager");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    LanguageMeta { slug: "swift", display_name: "Swift", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "swift-package-manager", display_name: "Swift Package Manager", aliases: &["spm", "swift-pm"] }
}

pub struct PackageSwiftParser;

impl ManifestParser for PackageSwiftParser {
    fn filenames(&self) -> &[&str] {
        &["Package.swift"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Must contain a Package definition
        if !content.contains("Package(") {
            return None;
        }

        // Parse name from `name: "..."` in the Package(...) definition
        let name = extract_swift_string(content, "name").unwrap_or_else(|| "app".to_string());

        // Parse swift-tools-version from comment at top of file
        let swift_version = extract_tools_version(content);

        // Detect if this is an executable (contains .executableTarget)
        let is_application = content.contains(".executableTarget");

        let dependencies = parse_swift_deps(content);

        // Extract the executable target name — it may differ from the package name.
        let exec_target_name =
            extract_executable_target_name(content).unwrap_or_else(|| name.clone());
        let entrypoint = if is_application {
            Some(format!("/app/{}", exec_target_name))
        } else {
            None
        };

        // Choose Swift Docker image based on swift-tools-version.
        // Swift is tightly coupled to the host's glibc headers — newer glibc (Wolfi)
        // breaks older swift-log due to localtime_r nullability changes.
        // Using the official Swift Docker images avoids these ABI issues.
        let swift_image = match swift_version.as_deref() {
            Some(v) if v.starts_with('5') => "docker.io/library/swift:5.10-jammy",
            _ => "docker.io/library/swift:6.1-noble",
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language: SWIFT,
            build_system: SWIFT_PM,
            runtime: NATIVE,
            package: Some(Package {
                name: name.clone(),
                version: swift_version,
                is_application,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![],
                commands: vec!["swift build -c release".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".build".into()],
                artifacts: vec![
                    (".build/release/".into(), "/app/".into()),
                    (
                        "/usr/lib/swift/linux/".into(),
                        "/usr/lib/swift/linux/".into(),
                    ),
                ],
                setup_commands: vec![],
                build_image: Some(swift_image.to_string()),
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "libstdc++".into(), "ca-certificates".into()],
                env: BTreeMap::from([("LD_LIBRARY_PATH".into(), "/usr/lib/swift/linux".into())]),
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

/// Extract a string value from a Swift source pattern like `name: "value"`.
fn extract_swift_string(content: &str, key: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(r#"{}:\s*"([^"]+)""#, regex::escape(key))).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Extract the swift-tools-version from the `// swift-tools-version:` comment.
fn extract_tools_version(content: &str) -> Option<String> {
    for line in content.lines().take(5) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("// swift-tools-version:") {
            let version = rest.trim();
            if !version.is_empty() {
                return Some(version.to_string());
            }
        }
    }
    None
}

/// Parse dependency package names from Swift Package.swift.
/// Matches `.package(url: "...", ...)` or `.package(name: "...", ...)`.
fn parse_swift_deps(content: &str) -> Vec<Dependency> {
    let re = match regex::Regex::new(r#"\.package\s*\([^)]*url:\s*"([^"]+)""#) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    re.captures_iter(content)
        .map(|cap| {
            let url = &cap[1];
            // Extract repo name from URL (e.g., "https://github.com/vapor/vapor.git" -> "vapor")
            let name = url
                .rsplit('/')
                .next()
                .unwrap_or(url)
                .trim_end_matches(".git")
                .to_string();
            Dependency {
                name,
                version: None,
                scope: DepScope::Runtime,
                is_internal: false,
            }
        })
        .collect()
}

/// Extract the first `.executableTarget(name: "...")` target name.
fn extract_executable_target_name(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"\.executableTarget\s*\(\s*name:\s*"([^"]+)""#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(PackageSwiftParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_package_swift_executable() {
        let parser = PackageSwiftParser;
        let content = r#"// swift-tools-version: 5.4
import PackageDescription
let package = Package(
    name: "swift",
    dependencies: [],
    targets: [
        .executableTarget(name: "swift", dependencies: []),
    ]
)
"#;
        let manifest = parser.parse(Path::new("Package.swift"), content).unwrap();
        assert_eq!(manifest.language, LanguageId::new("swift"));
        assert_eq!(
            manifest.build_system,
            BuildSystemId::new("swift-package-manager")
        );
        assert_eq!(manifest.runtime, RuntimeId::new("native"));
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "swift");
        assert_eq!(pkg.version.as_deref(), Some("5.4"));
        assert!(pkg.is_application);
        assert_eq!(
            manifest.runtime_config.entrypoint.as_deref(),
            Some("/app/swift")
        );
        assert_eq!(manifest.runtime_config.ports, vec![8080]);
        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("swift build -c release")));
        assert!(manifest.build.setup_commands.is_empty());
        assert_eq!(
            manifest.build.build_image.as_deref(),
            Some("docker.io/library/swift:5.10-jammy")
        );
    }

    #[test]
    fn test_package_swift_library() {
        let parser = PackageSwiftParser;
        let content = r#"// swift-tools-version: 5.9
import PackageDescription
let package = Package(
    name: "my-lib",
    targets: [
        .target(name: "MyLib"),
    ]
)
"#;
        let manifest = parser.parse(Path::new("Package.swift"), content).unwrap();
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-lib");
        assert!(!pkg.is_application);
        // Library has no entrypoint
        assert!(manifest.runtime_config.entrypoint.is_none());
    }

    #[test]
    fn test_package_swift_with_deps() {
        let parser = PackageSwiftParser;
        let content = r#"// swift-tools-version: 5.7
import PackageDescription
let package = Package(
    name: "my-server",
    dependencies: [
        .package(url: "https://github.com/vapor/vapor.git", from: "4.0.0"),
        .package(url: "https://github.com/vapor/fluent.git", from: "4.0.0"),
    ],
    targets: [
        .executableTarget(name: "my-server", dependencies: ["Vapor", "Fluent"]),
    ]
)
"#;
        let manifest = parser.parse(Path::new("Package.swift"), content).unwrap();
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].name, "vapor");
        assert_eq!(manifest.dependencies[1].name, "fluent");
    }

    #[test]
    fn test_extract_tools_version() {
        assert_eq!(
            extract_tools_version("// swift-tools-version: 5.4\nimport PackageDescription"),
            Some("5.4".to_string())
        );
        assert_eq!(
            extract_tools_version("// swift-tools-version:5.9\n"),
            Some("5.9".to_string())
        );
    }

    #[test]
    fn test_not_a_package() {
        let parser = PackageSwiftParser;
        let content = "// Just a regular Swift file\nprint(\"hello\")";
        assert!(parser.parse(Path::new("Package.swift"), content).is_none());
    }
}
