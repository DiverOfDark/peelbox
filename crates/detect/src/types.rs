//! Core types for the map-reduce detection pipeline.
//!
//! All manifest formats normalize into generic `Manifest` and `ConfigContribution` types.
//! No per-format structs are exposed outside the parsers.

use crate::ids::{BuildSystemId, FrameworkId, LanguageId, OrchestratorId, RuntimeId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

// ── Tree types ──────────────────────────────────────────────────────────────

/// The complete parsed representation of a repository.
#[derive(Debug)]
pub struct RepoTree {
    pub root: PathBuf,
    pub tree: DirNode,
}

/// A directory in the tree.
#[derive(Debug)]
pub struct DirNode {
    /// Relative path from repo root (empty for root).
    pub path: PathBuf,
    /// Typed files directly in this directory.
    pub files: Vec<TypedFile>,
    /// Child directories (recursive).
    pub children: Vec<DirNode>,
}

/// A file parsed into its typed representation.
#[derive(Debug)]
pub struct TypedFile {
    pub path: PathBuf,
    pub kind: FileKind,
}

/// File classification — each variant carries structured, already-reduced data.
#[derive(Debug)]
pub enum FileKind {
    /// A build manifest, normalized to a generic representation.
    Manifest(Box<Manifest>),
    /// A configuration file that contributes runtime config.
    Config(ConfigContribution),
    /// A source code file.
    Source { language: LanguageId },
    /// Unrecognized file.
    Other,
}

// ── Manifest types ──────────────────────────────────────────────────────────

/// Universal manifest — every build system format normalizes to this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Source file path (relative to repo root).
    pub path: PathBuf,
    /// Detected language.
    pub language: LanguageId,
    /// Detected build system.
    pub build_system: BuildSystemId,
    /// Detected runtime.
    pub runtime: RuntimeId,
    /// Package metadata (if this manifest defines a package).
    pub package: Option<Package>,
    /// Workspace definition (if this is a workspace root).
    pub workspace: Option<Workspace>,
    /// Dependencies declared in this manifest.
    pub dependencies: Vec<Dependency>,
    /// Build specification.
    pub build: BuildSpec,
    /// Runtime specification.
    pub runtime_config: RuntimeSpec,
}

/// Universal package metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: Option<String>,
    pub is_application: bool,
}

/// Universal workspace definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Member patterns — can be globs ("crates/*") or exact paths ("core", "web").
    pub members: Vec<String>,
    /// Monorepo orchestrator if detected (Turborepo, Nx, Lerna).
    pub orchestrator: Option<OrchestratorId>,
}

/// Universal dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub scope: DepScope,
    pub is_internal: bool,
}

/// Dependency scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DepScope {
    #[default]
    Runtime,
    Dev,
    Build,
    Peer,
}

/// Build specification — what's needed to build this project.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildSpec {
    /// Wolfi packages for build stage.
    pub packages: Vec<String>,
    /// Base commands (for root/standalone projects).
    pub commands: Vec<String>,
    /// How to transform commands when this is a workspace member.
    pub member_transform: Option<MemberBuildTransform>,
    /// Build environment variables.
    pub env: BTreeMap<String, String>,
    /// Directories to cache between builds.
    pub cache_dirs: Vec<String>,
    /// Artifacts to copy from build → runtime (from, to).
    pub artifacts: Vec<(String, String)>,
}

/// How to adjust build commands for a workspace member.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberBuildTransform {
    /// Template commands for when this service is a workspace member.
    /// `{module}` is replaced with the module's relative path or name.
    pub member_commands: Vec<String>,
    /// Template artifacts for workspace members.
    /// `{module}` is replaced similarly.
    pub member_artifacts: Option<Vec<(String, String)>>,
}

/// Runtime specification — what's needed to run this project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSpec {
    /// Wolfi packages for runtime.
    pub packages: Vec<String>,
    /// Runtime environment variables.
    pub env: BTreeMap<String, String>,
    /// Container entrypoint command.
    pub entrypoint: Option<String>,
    /// Working directory.
    pub workdir: Option<String>,
    /// Exposed ports.
    pub ports: Vec<u16>,
    /// Health check endpoint.
    pub health_endpoint: Option<String>,
}

// ── Config contribution types ───────────────────────────────────────────────

/// Supplementary configuration extracted from non-manifest files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigContribution {
    /// Source file path (relative to repo root).
    pub path: PathBuf,
    /// Additional environment variables discovered.
    pub env_vars: BTreeMap<String, String>,
    /// Additional ports discovered.
    pub ports: Vec<u16>,
    /// Health endpoint discovered.
    pub health_endpoint: Option<String>,
}

// ── Framework contribution ──────────────────────────────────────────────────

/// What a detected framework adds to the service.
#[derive(Debug, Clone)]
pub struct FrameworkContribution {
    pub framework: FrameworkId,
    pub default_ports: Vec<u16>,
    pub health_endpoints: Vec<String>,
    pub env_vars: BTreeMap<String, String>,
    pub runtime_packages: Vec<String>,
    /// Optional runtime command override (e.g., ["flask", "run"])
    pub runtime_command: Option<Vec<String>>,
    /// Optional runtime env var overrides
    pub runtime_env: BTreeMap<String, String>,
    /// Optional workdir override
    pub workdir: Option<String>,
    /// Optional additional copy specs
    pub extra_copy: Vec<(String, String)>,
}

// ── Service bucket ──────────────────────────────────────────────────────────

/// A service with all its accumulated data, ready for final assembly.
#[derive(Debug)]
pub struct ServiceBucket {
    /// Service root path relative to repo root.
    pub path: PathBuf,
    /// The primary manifest defining this service.
    pub manifest: Manifest,
    /// Config contributions from files in this service's scope.
    pub configs: Vec<ConfigContribution>,
    /// Detected framework (if any).
    pub framework: Option<FrameworkContribution>,
    /// Whether this is a member of a workspace (vs standalone).
    pub is_workspace_member: bool,
    /// The workspace root path (if this is a workspace member).
    pub workspace_root: Option<PathBuf>,
}

impl ServiceBucket {
    /// Get the module name for workspace-aware command templates.
    pub fn module_name(&self) -> String {
        self.path.to_string_lossy().to_string()
    }

    /// Get the workspace root display path for command templates.
    pub fn workspace_root_display(&self) -> String {
        self.workspace_root
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string())
    }
}

// ── Default impls ───────────────────────────────────────────────────────────

impl Default for RuntimeSpec {
    fn default() -> Self {
        Self {
            packages: Vec::new(),
            env: BTreeMap::new(),
            entrypoint: None,
            workdir: Some("/app".to_string()),
            ports: Vec::new(),
            health_endpoint: None,
        }
    }
}
