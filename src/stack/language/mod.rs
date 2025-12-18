mod cpp;
mod dotnet;
mod elixir;
mod go;
mod java;
mod javascript;
pub mod parsers;
mod php;
mod python;
mod ruby;
mod rust;

pub use cpp::CppLanguage;
pub use dotnet::DotNetLanguage;
pub use elixir::ElixirLanguage;
pub use go::GoLanguage;
pub use java::JavaLanguage;
pub use javascript::JavaScriptLanguage;
pub use php::PhpLanguage;
pub use python::PythonLanguage;
pub use ruby::RubyLanguage;
pub use rust::RustLanguage;

#[derive(Debug, Clone, serde::Serialize)]
pub struct BuildTemplate {
    pub build_image: String,
    pub runtime_image: String,
    pub build_packages: Vec<String>,
    pub runtime_packages: Vec<String>,
    pub build_commands: Vec<String>,
    pub cache_paths: Vec<String>,
    pub artifacts: Vec<String>,
    pub common_ports: Vec<u16>,
}

pub trait LanguageDefinition: Send + Sync {
    fn id(&self) -> crate::stack::LanguageId;
    fn extensions(&self) -> &[&str];
    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult>;
    fn compatible_build_systems(&self) -> &[&str];

    fn excluded_dirs(&self) -> &[&str] {
        &[]
    }

    fn workspace_configs(&self) -> &[&str] {
        &[]
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

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![]
    }

    fn health_check_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![]
    }

    fn is_main_file(&self, _fs: &dyn crate::fs::FileSystem, _file_path: &std::path::Path) -> bool {
        false
    }

    fn default_health_endpoints(&self) -> Vec<(&'static str, &'static str)> {
        vec![]
    }

    fn default_env_vars(&self) -> Vec<&'static str> {
        vec![]
    }

    fn port_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![]
    }

    fn runtime_name(&self) -> Option<&'static str> {
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
}

#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub build_system: crate::stack::BuildSystemId,
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
