//! Scanner: unified entry point for project detection.
//!
//! Replaces the fragmented detection flow (scan → enrich → dedup → identify stack)
//! with a single pass that produces fully-enriched `DetectedProject` results.

use crate::registry::StackRegistry;
use crate::{BuildSystemId, DetectionStack, FrameworkId, LanguageId, RuntimeId};
use anyhow::Result;
use peelbox_core::config::DetectionMode;
use peelbox_core::fs::FileSystem;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Shared context passed through the scan process.
pub struct ScanContext<'a> {
    pub repo_root: &'a Path,
    pub file_tree: &'a [PathBuf],
    pub fs: &'a dyn FileSystem,
    pub detection_mode: DetectionMode,
    pub registry: &'a StackRegistry,
}

/// Fully-detected project — richer than `DetectionStack`.
///
/// Includes everything needed downstream: runtime, framework, version.
/// This is what `Scanner::scan()` produces.
#[derive(Debug, Clone)]
pub struct DetectedProject {
    pub manifest_path: PathBuf,
    pub build_system: BuildSystemId,
    pub language: LanguageId,
    pub runtime: RuntimeId,
    pub framework: Option<FrameworkId>,
    pub version: Option<String>,
    pub confidence: f64,
    pub depth: usize,
    pub is_workspace_root: bool,
}

impl DetectedProject {
    /// Convert to a `DetectionStack` for backward compatibility with downstream phases.
    pub fn to_detection_stack(&self) -> DetectionStack {
        DetectionStack::new(
            self.build_system.clone(),
            self.language.clone(),
            self.manifest_path.clone(),
        )
        .with_confidence(self.confidence)
        .with_depth(self.depth)
        .with_workspace_root(self.is_workspace_root)
    }
}

/// Single entry point for project detection (the "Decision Tree").
///
/// Replaces `StackRegistry::detect_all_stacks()` + inline enrichment in `ScanPhase`.
pub struct Scanner;

impl Scanner {
    /// Scan a repository and return fully-enriched project detections.
    ///
    /// Flow:
    /// 1. Run static build systems (unless LLMOnly mode)
    /// 2. Run LLM fallback if needed (unless StaticOnly mode)
    /// 3. Enrich each detection with depth, workspace_root, runtime, framework, version
    /// 4. Deduplicate per directory
    pub fn scan(ctx: &ScanContext) -> Result<Vec<DetectedProject>> {
        let raw_detections = Self::detect_all(ctx)?;
        let enriched = Self::enrich(ctx, raw_detections);
        let deduped = Self::deduplicate(ctx.registry, enriched);
        Ok(deduped)
    }

    /// Run all build systems and collect raw detections.
    fn detect_all(ctx: &ScanContext) -> Result<Vec<DetectionStack>> {
        ctx.registry
            .detect_all_stacks(ctx.repo_root, ctx.file_tree, ctx.fs, ctx.detection_mode)
    }

    /// Enrich raw detections with runtime, framework, version, depth, workspace_root.
    fn enrich(ctx: &ScanContext, detections: Vec<DetectionStack>) -> Vec<DetectedProject> {
        detections
            .into_iter()
            .map(|d| Self::enrich_one(ctx, d))
            .collect()
    }

    fn enrich_one(ctx: &ScanContext, detection: DetectionStack) -> DetectedProject {
        let rel_path = detection
            .manifest_path
            .strip_prefix(ctx.repo_root)
            .unwrap_or(&detection.manifest_path);
        let depth = rel_path.to_string_lossy().matches('/').count();

        // Determine runtime from build system, fall back to language profile
        let runtime = ctx
            .registry
            .get_build_system(detection.build_system.clone())
            .and_then(|bs| bs.runtime_id())
            .or_else(|| {
                ctx.registry
                    .get_language(detection.language.clone())
                    .and_then(|lang| {
                        let rt_name = lang.runtime_name()?;
                        RuntimeId::from_name(&rt_name)
                    })
            })
            .unwrap_or(RuntimeId::Native);

        // Check workspace root via build system
        let manifest_abs = ctx.repo_root.join(&detection.manifest_path);
        let content = ctx.fs.read_to_string(&manifest_abs).ok();
        let is_workspace_root = ctx
            .registry
            .is_workspace_root_by_build_system(&detection.build_system, content.as_deref());

        // Detect version via language
        let version = ctx
            .registry
            .get_language(detection.language.clone())
            .and_then(|lang| lang.detect_version(content.as_deref()));

        // Detect framework via dependency analysis
        let framework = Self::detect_framework(ctx, &detection.language, &detection.manifest_path);

        DetectedProject {
            manifest_path: detection.manifest_path,
            build_system: detection.build_system,
            language: detection.language,
            runtime,
            framework,
            version,
            confidence: detection.confidence,
            depth,
            is_workspace_root,
        }
    }

    /// Detect framework by parsing dependencies and matching framework patterns.
    fn detect_framework(
        ctx: &ScanContext,
        language: &LanguageId,
        manifest_path: &Path,
    ) -> Option<FrameworkId> {
        let manifest_abs = ctx.repo_root.join(manifest_path);
        let manifest_content = ctx.fs.read_to_string(&manifest_abs).ok()?;

        let project_dir = manifest_path.parent().unwrap_or(Path::new(""));
        let dep_info = ctx.registry.parse_dependencies_for_language(
            language,
            &manifest_content,
            std::slice::from_ref(&project_dir.to_path_buf()),
        )?;

        for fw_id in crate::FrameworkId::all_variants() {
            if let Some(fw) = ctx.registry.get_framework(fw_id.clone()) {
                let patterns = fw.dependency_patterns();
                for pattern in &patterns {
                    if dep_info.external_deps.iter().any(|d| pattern.matches(d))
                        || dep_info.internal_deps.iter().any(|d| pattern.matches(d))
                    {
                        return Some(fw_id.clone());
                    }
                }
            }
        }

        None
    }

    /// Deduplicate detections: keep highest-priority manifest per directory.
    fn deduplicate(
        registry: &StackRegistry,
        detections: Vec<DetectedProject>,
    ) -> Vec<DetectedProject> {
        let mut by_directory: HashMap<PathBuf, Vec<DetectedProject>> = HashMap::new();

        for detection in detections {
            let dir = detection
                .manifest_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf();
            by_directory.entry(dir).or_default().push(detection);
        }

        let mut deduplicated = Vec::new();

        for (_dir, mut detections_for_dir) in by_directory {
            if detections_for_dir.len() == 1 {
                deduplicated.extend(detections_for_dir);
            } else {
                detections_for_dir.sort_by_key(|d| {
                    let build_system = registry.get_build_system(d.build_system.clone());
                    let priority = build_system
                        .and_then(|bs| {
                            bs.manifest_patterns()
                                .iter()
                                .find(|p| {
                                    d.manifest_path.file_name()
                                        == Some(std::ffi::OsStr::new(&p.filename))
                                })
                                .map(|p| p.priority)
                        })
                        .unwrap_or(0);
                    std::cmp::Reverse(priority)
                });

                if let Some(highest_priority) = detections_for_dir.into_iter().next() {
                    deduplicated.push(highest_priority);
                }
            }
        }

        deduplicated.sort_by(|a, b| a.manifest_path.cmp(&b.manifest_path));
        deduplicated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peelbox_core::fs::{DirEntry, FileMetadata, FileType};
    use std::collections::HashMap as StdHashMap;

    struct MockFs {
        files: StdHashMap<PathBuf, String>,
    }

    impl FileSystem for MockFs {
        fn read_to_string(&self, path: &Path) -> Result<String, anyhow::Error> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("not found"))
        }
        fn exists(&self, path: &Path) -> bool {
            self.files.contains_key(path)
        }
        fn is_file(&self, path: &Path) -> bool {
            self.exists(path)
        }
        fn is_dir(&self, _path: &Path) -> bool {
            false
        }
        fn read_dir(&self, _path: &Path) -> Result<Vec<DirEntry>, anyhow::Error> {
            Ok(vec![])
        }
        fn metadata(&self, path: &Path) -> Result<FileMetadata, anyhow::Error> {
            if self.exists(path) {
                Ok(FileMetadata {
                    size: 100,
                    file_type: FileType::File,
                })
            } else {
                Err(anyhow::anyhow!("not found"))
            }
        }
        fn read_bytes(&self, path: &Path, _max_bytes: usize) -> Result<Vec<u8>, anyhow::Error> {
            self.read_to_string(path).map(|s| s.into_bytes())
        }
        fn canonicalize(&self, path: &Path) -> Result<PathBuf, anyhow::Error> {
            Ok(path.to_path_buf())
        }
    }

    #[test]
    fn test_scanner_scan_cargo() {
        let repo_root = PathBuf::from("/repo");
        let mut files = StdHashMap::new();
        files.insert(
            PathBuf::from("/repo/Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"".to_string(),
        );
        files.insert(PathBuf::from("/repo/src/main.rs"), "fn main() {}".to_string());
        let fs = MockFs { files };
        let file_tree = vec![
            PathBuf::from("Cargo.toml"),
            PathBuf::from("src/main.rs"),
        ];

        let registry = StackRegistry::with_defaults(None);
        let ctx = ScanContext {
            repo_root: &repo_root,
            file_tree: &file_tree,
            fs: &fs,
            detection_mode: DetectionMode::StaticOnly,
            registry: &registry,
        };

        let results = Scanner::scan(&ctx).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].build_system, BuildSystemId::Cargo);
        assert_eq!(results[0].language, LanguageId::Rust);
        assert_eq!(results[0].runtime, RuntimeId::Native);
        assert_eq!(results[0].depth, 0);
    }

    #[test]
    fn test_scanner_deduplication() {
        let registry = StackRegistry::with_defaults(None);

        let projects = vec![
            DetectedProject {
                manifest_path: PathBuf::from("Cargo.toml"),
                build_system: BuildSystemId::Cargo,
                language: LanguageId::Rust,
                runtime: RuntimeId::Native,
                framework: None,
                version: None,
                confidence: 1.0,
                depth: 0,
                is_workspace_root: false,
            },
            DetectedProject {
                manifest_path: PathBuf::from("package.json"),
                build_system: BuildSystemId::Npm,
                language: LanguageId::JavaScript,
                runtime: RuntimeId::Node,
                framework: None,
                version: None,
                confidence: 1.0,
                depth: 0,
                is_workspace_root: false,
            },
        ];

        let deduped = Scanner::deduplicate(&registry, projects);
        // Both in same directory (root) — only highest priority kept
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn test_detected_project_to_detection_stack() {
        let project = DetectedProject {
            manifest_path: PathBuf::from("pom.xml"),
            build_system: BuildSystemId::Maven,
            language: LanguageId::Java,
            runtime: RuntimeId::JVM,
            framework: Some(FrameworkId::SpringBoot),
            version: Some("17".to_string()),
            confidence: 0.95,
            depth: 1,
            is_workspace_root: false,
        };

        let stack = project.to_detection_stack();
        assert_eq!(stack.build_system, BuildSystemId::Maven);
        assert_eq!(stack.language, LanguageId::Java);
        assert_eq!(stack.confidence, 0.95);
        assert_eq!(stack.depth, 1);
        assert!(!stack.is_workspace_root);
    }
}
