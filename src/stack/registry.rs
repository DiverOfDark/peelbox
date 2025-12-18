use super::{BuildSystemId, DetectionStack, FrameworkId, LanguageId, OrchestratorId};
use crate::stack::buildsystem::*;
use crate::stack::framework::*;
use crate::stack::language::*;
use crate::stack::orchestrator::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub struct StackRegistry {
    build_systems: HashMap<BuildSystemId, Arc<dyn BuildSystem>>,
    languages: HashMap<LanguageId, Arc<dyn LanguageDefinition>>,
    frameworks: HashMap<FrameworkId, Box<dyn Framework>>,
    orchestrators: HashMap<OrchestratorId, Arc<dyn MonorepoOrchestrator>>,
}

impl StackRegistry {
    pub fn new() -> Self {
        Self {
            build_systems: HashMap::new(),
            languages: HashMap::new(),
            frameworks: HashMap::new(),
            orchestrators: HashMap::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();

        registry.register_language(Arc::new(RustLanguage));
        registry.register_language(Arc::new(JavaLanguage));
        registry.register_language(Arc::new(JavaScriptLanguage));
        registry.register_language(Arc::new(PythonLanguage));
        registry.register_language(Arc::new(GoLanguage));
        registry.register_language(Arc::new(DotNetLanguage));
        registry.register_language(Arc::new(RubyLanguage));
        registry.register_language(Arc::new(PhpLanguage));
        registry.register_language(Arc::new(CppLanguage));
        registry.register_language(Arc::new(ElixirLanguage));

        registry.register_build_system(Arc::new(CargoBuildSystem));
        registry.register_build_system(Arc::new(MavenBuildSystem));
        registry.register_build_system(Arc::new(GradleBuildSystem));
        registry.register_build_system(Arc::new(NpmBuildSystem));
        registry.register_build_system(Arc::new(YarnBuildSystem));
        registry.register_build_system(Arc::new(PnpmBuildSystem));
        registry.register_build_system(Arc::new(BunBuildSystem));
        registry.register_build_system(Arc::new(PipBuildSystem));
        registry.register_build_system(Arc::new(PoetryBuildSystem));
        registry.register_build_system(Arc::new(PipenvBuildSystem));
        registry.register_build_system(Arc::new(GoModBuildSystem));
        registry.register_build_system(Arc::new(DotNetBuildSystem));
        registry.register_build_system(Arc::new(ComposerBuildSystem));
        registry.register_build_system(Arc::new(BundlerBuildSystem));
        registry.register_build_system(Arc::new(CMakeBuildSystem));
        registry.register_build_system(Arc::new(MakeBuildSystem));
        registry.register_build_system(Arc::new(MesonBuildSystem));
        registry.register_build_system(Arc::new(MixBuildSystem));

        registry.register_framework(Box::new(SpringBootFramework));
        registry.register_framework(Box::new(QuarkusFramework));
        registry.register_framework(Box::new(MicronautFramework));
        registry.register_framework(Box::new(KtorFramework));
        registry.register_framework(Box::new(ExpressFramework));
        registry.register_framework(Box::new(NextJsFramework));
        registry.register_framework(Box::new(NestJsFramework));
        registry.register_framework(Box::new(FastifyFramework));
        registry.register_framework(Box::new(DjangoFramework));
        registry.register_framework(Box::new(FlaskFramework));
        registry.register_framework(Box::new(FastApiFramework));
        registry.register_framework(Box::new(RailsFramework));
        registry.register_framework(Box::new(SinatraFramework));
        registry.register_framework(Box::new(ActixFramework));
        registry.register_framework(Box::new(AxumFramework));
        registry.register_framework(Box::new(GinFramework));
        registry.register_framework(Box::new(EchoFramework));
        registry.register_framework(Box::new(AspNetFramework));
        registry.register_framework(Box::new(LaravelFramework));
        registry.register_framework(Box::new(PhoenixFramework));

        registry.register_orchestrator(Arc::new(TurborepoOrchestrator));
        registry.register_orchestrator(Arc::new(NxOrchestrator));
        registry.register_orchestrator(Arc::new(LernaOrchestrator));

        registry
    }

    pub fn register_build_system(&mut self, build_system: Arc<dyn BuildSystem>) {
        let id = build_system.id();
        self.build_systems.insert(id, build_system);
    }

    pub fn register_language(&mut self, language: Arc<dyn LanguageDefinition>) {
        let id = language.id();
        self.languages.insert(id, language);
    }

    pub fn register_framework(&mut self, framework: Box<dyn Framework>) {
        let id = framework.id();
        self.frameworks.insert(id, framework);
    }

    pub fn get_build_system(&self, id: BuildSystemId) -> Option<&dyn BuildSystem> {
        self.build_systems.get(&id).map(|bs| bs.as_ref())
    }

    pub fn get_language(&self, id: LanguageId) -> Option<&dyn LanguageDefinition> {
        self.languages.get(&id).map(|l| l.as_ref())
    }

    pub fn get_framework(&self, id: FrameworkId) -> Option<&dyn Framework> {
        self.frameworks.get(&id).map(|f| f.as_ref())
    }

    pub fn register_orchestrator(&mut self, orchestrator: Arc<dyn MonorepoOrchestrator>) {
        self.orchestrators.insert(orchestrator.id(), orchestrator);
    }

    pub fn get_orchestrator(&self, id: OrchestratorId) -> Option<&dyn MonorepoOrchestrator> {
        self.orchestrators.get(&id).map(|o| o.as_ref())
    }

    pub fn all_orchestrators(&self) -> Vec<&dyn MonorepoOrchestrator> {
        self.orchestrators.values().map(|o| o.as_ref()).collect()
    }

    pub fn detect_build_system_opt(
        &self,
        manifest_path: &Path,
        content: Option<&str>,
    ) -> Option<BuildSystemId> {
        let filename = manifest_path.file_name()?.to_str()?;

        for (id, build_system) in &self.build_systems {
            if build_system.detect(filename, content) {
                return Some(*id);
            }
        }
        None
    }

    pub fn detect_build_system(
        &self,
        manifest_path: &Path,
        content: &str,
    ) -> Option<BuildSystemId> {
        self.detect_build_system_opt(manifest_path, Some(content))
    }

    pub fn detect_language_opt(
        &self,
        manifest_path: &Path,
        content: Option<&str>,
        build_system: BuildSystemId,
    ) -> Option<LanguageId> {
        let filename = manifest_path.file_name()?.to_str()?;

        for (id, language) in &self.languages {
            if let Some(result) = language.detect(filename, content) {
                if result.build_system == build_system {
                    return Some(*id);
                }
            }
        }
        None
    }

    pub fn detect_language(
        &self,
        manifest_path: &Path,
        content: &str,
        build_system: BuildSystemId,
    ) -> Option<LanguageId> {
        self.detect_language_opt(manifest_path, Some(content), build_system)
    }

    pub fn detect_stack_opt(
        &self,
        manifest_path: &Path,
        content: Option<&str>,
    ) -> Option<DetectionStack> {
        let build_system = self.detect_build_system_opt(manifest_path, content)?;
        let language = self.detect_language_opt(manifest_path, content, build_system)?;

        Some(DetectionStack::new(
            build_system,
            language,
            manifest_path.to_path_buf(),
        ))
    }

    pub fn detect_stack(&self, manifest_path: &Path, content: &str) -> Option<DetectionStack> {
        self.detect_stack_opt(manifest_path, Some(content))
    }

    pub fn is_manifest(&self, filename: &str) -> bool {
        for build_system in self.build_systems.values() {
            if build_system.detect(filename, None) {
                return true;
            }
        }
        false
    }

    pub fn all_excluded_dirs(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for lang in self.languages.values() {
            for dir in lang.excluded_dirs() {
                if seen.insert(*dir) {
                    result.push(*dir);
                }
            }
        }
        result
    }

    pub fn all_workspace_configs(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for lang in self.languages.values() {
            for config in lang.workspace_configs() {
                if seen.insert(*config) {
                    result.push(*config);
                }
            }
        }
        result
    }

    pub fn is_workspace_root(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        for language in self.languages.values() {
            if language.is_workspace_root(manifest_name, manifest_content) {
                return true;
            }
        }
        false
    }

    pub fn parse_dependencies_by_manifest(
        &self,
        manifest_name: &str,
        manifest_content: &str,
        all_internal_paths: &[std::path::PathBuf],
    ) -> Option<crate::stack::language::DependencyInfo> {
        for language in self.languages.values() {
            if language
                .detect(manifest_name, Some(manifest_content))
                .is_some()
            {
                return Some(language.parse_dependencies(manifest_content, all_internal_paths));
            }
        }
        None
    }
}

impl Default for StackRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
