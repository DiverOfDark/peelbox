mod cpp;
mod dotnet;
mod elixir;
mod go;
mod java;
mod javascript;
pub mod llm;
pub mod parsers;
mod php;
mod python;
mod ruby;
mod rust;
mod zig;

pub use cpp::CppLanguage;
pub use dotnet::DotNetLanguage;
pub use elixir::ElixirLanguage;
pub use go::GoLanguage;
pub use java::JavaLanguage;
pub use javascript::JavaScriptLanguage;
pub use llm::LLMLanguage;
pub use php::PhpLanguage;
pub use python::PythonLanguage;
pub use ruby::RubyLanguage;
pub use rust::RustLanguage;
pub use zig::ZigLanguage;

pub trait LanguageDefinition: Send + Sync {
    fn id(&self) -> crate::LanguageId;
    fn extensions(&self) -> Vec<String>;
    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult>;
    fn compatible_build_systems(&self) -> Vec<String>;

    fn excluded_dirs(&self) -> Vec<String> {
        vec![]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }

    fn detect_version(&self, _manifest_content: Option<&str>) -> Option<String> {
        None
    }

    fn is_workspace_root(&self, _manifest_name: &str, _manifest_content: Option<&str>) -> bool {
        false
    }

    fn parse_dependencies(
        &self,
        _manifest_content: &str,
        _all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        DependencyInfo::empty()
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![]
    }

    fn health_check_patterns(&self) -> Vec<(String, String)> {
        vec![]
    }

    fn is_main_file(
        &self,
        _fs: &dyn peelbox_core::fs::FileSystem,
        _file_path: &std::path::Path,
    ) -> bool {
        false
    }

    fn default_health_endpoints(&self) -> Vec<(String, String)> {
        vec![]
    }

    fn default_env_vars(&self) -> Vec<String> {
        vec![]
    }

    fn port_patterns(&self) -> Vec<(String, String)> {
        vec![]
    }

    fn runtime_name(&self) -> Option<String> {
        None
    }

    fn default_port(&self) -> Option<u16> {
        None
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        None
    }

    fn parse_entrypoint_from_manifest(&self, _manifest_content: &str) -> Option<String> {
        None
    }

    fn find_entrypoints(
        &self,
        _fs: &dyn peelbox_core::fs::FileSystem,
        _repo_root: &std::path::Path,
        _project_root: &std::path::Path,
        _file_tree: &[std::path::PathBuf],
    ) -> Vec<String> {
        vec![]
    }

    fn is_runnable(
        &self,
        _fs: &dyn peelbox_core::fs::FileSystem,
        _repo_root: &std::path::Path,
        _project_root: &std::path::Path,
        _file_tree: &[std::path::PathBuf],
        _manifest_content: Option<&str>,
    ) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub build_system: crate::BuildSystemId,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionMethod {
    Deterministic,
    LLM,
    NotImplemented,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub is_internal: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DependencyInfo {
    pub internal_deps: Vec<Dependency>,
    pub external_deps: Vec<Dependency>,
    pub detected_by: DetectionMethod,
}

impl DependencyInfo {
    pub fn empty() -> Self {
        Self {
            internal_deps: vec![],
            external_deps: vec![],
            detected_by: DetectionMethod::NotImplemented,
        }
    }
}
