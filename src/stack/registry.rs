use super::{BuildSystemId, DetectionStack, FrameworkId, LanguageId, OrchestratorId, RuntimeId};
use crate::llm::LLMClient;
use crate::stack::buildsystem::*;
use crate::stack::framework::*;
use crate::stack::language::*;
use crate::stack::orchestrator::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Registry for all technology stack components.
///
/// Provides unified detection and lookup for languages, build systems, frameworks,
/// orchestrators, and runtimes. Supports both deterministic pattern matching and
/// LLM-based fallback for unknown technologies.
///
/// # Registration Order
///
/// Components are registered in priority order:
/// 1. Known deterministic implementations (Rust, Java, npm, etc.)
/// 2. LLM-backed fallback implementations (if llm_client provided)
///
/// This ensures fast, reliable detection for known tech while enabling discovery
/// of unknown technologies via LLM inference.
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

    /// Creates a registry pre-populated with default implementations.
    ///
    /// # Arguments
    ///
    /// * `llm_client` - Optional LLM client for fallback detection. If provided, LLM-backed
    ///                  implementations are registered last to handle unknown technologies.
    ///
    /// # Registration Order
    ///
    /// 1. All known languages (Rust, Java, JavaScript, Python, Go, etc.)
    /// 2. All known build systems (Cargo, Maven, Gradle, npm, etc.)
    /// 3. All known frameworks (Spring Boot, Next.js, Django, etc.)
    /// 4. All known orchestrators (Turborepo, Nx, Lerna)
    /// 5. LLM fallback implementations (if llm_client provided)
    ///
    /// This order ensures deterministic detection tries first, with LLM as last resort.
    pub fn with_defaults(llm_client: Option<Arc<dyn LLMClient>>) -> Self {
        let mut registry = Self::new();

        registry
            .languages
            .insert(LanguageId::Rust, Arc::new(RustLanguage));
        registry
            .languages
            .insert(LanguageId::Java, Arc::new(JavaLanguage));
        registry
            .languages
            .insert(LanguageId::JavaScript, Arc::new(JavaScriptLanguage));
        registry
            .languages
            .insert(LanguageId::Python, Arc::new(PythonLanguage));
        registry
            .languages
            .insert(LanguageId::Go, Arc::new(GoLanguage));
        registry
            .languages
            .insert(LanguageId::CSharp, Arc::new(DotNetLanguage));
        registry
            .languages
            .insert(LanguageId::Ruby, Arc::new(RubyLanguage));
        registry
            .languages
            .insert(LanguageId::PHP, Arc::new(PhpLanguage));
        registry
            .languages
            .insert(LanguageId::Cpp, Arc::new(CppLanguage));
        registry
            .languages
            .insert(LanguageId::Elixir, Arc::new(ElixirLanguage));

        for id in BuildSystemId::all_variants() {
            let bs: Arc<dyn BuildSystem> = match id {
                BuildSystemId::Cargo => Arc::new(CargoBuildSystem),
                BuildSystemId::Maven => Arc::new(MavenBuildSystem),
                BuildSystemId::Gradle => Arc::new(GradleBuildSystem),
                BuildSystemId::Npm => Arc::new(NpmBuildSystem),
                BuildSystemId::Yarn => Arc::new(YarnBuildSystem),
                BuildSystemId::Pnpm => Arc::new(PnpmBuildSystem),
                BuildSystemId::Bun => Arc::new(BunBuildSystem),
                BuildSystemId::Pip => Arc::new(PipBuildSystem),
                BuildSystemId::Poetry => Arc::new(PoetryBuildSystem),
                BuildSystemId::Pipenv => Arc::new(PipenvBuildSystem),
                BuildSystemId::GoMod => Arc::new(GoModBuildSystem),
                BuildSystemId::DotNet => Arc::new(DotNetBuildSystem),
                BuildSystemId::Composer => Arc::new(ComposerBuildSystem),
                BuildSystemId::Bundler => Arc::new(BundlerBuildSystem),
                BuildSystemId::CMake => Arc::new(CMakeBuildSystem),
                BuildSystemId::Make => Arc::new(MakeBuildSystem),
                BuildSystemId::Meson => Arc::new(MesonBuildSystem),
                BuildSystemId::Mix => Arc::new(MixBuildSystem),
                BuildSystemId::Custom(_) => continue,
            };
            registry.build_systems.insert(id.clone(), bs);
        }

        for id in FrameworkId::all_variants() {
            let fw: Box<dyn Framework> = match id {
                FrameworkId::SpringBoot => Box::new(SpringBootFramework),
                FrameworkId::Quarkus => Box::new(QuarkusFramework),
                FrameworkId::Micronaut => Box::new(MicronautFramework),
                FrameworkId::Ktor => Box::new(KtorFramework),
                FrameworkId::Express => Box::new(ExpressFramework),
                FrameworkId::NextJs => Box::new(NextJsFramework),
                FrameworkId::NestJs => Box::new(NestJsFramework),
                FrameworkId::Fastify => Box::new(FastifyFramework),
                FrameworkId::Django => Box::new(DjangoFramework),
                FrameworkId::Flask => Box::new(FlaskFramework),
                FrameworkId::FastApi => Box::new(FastApiFramework),
                FrameworkId::Rails => Box::new(RailsFramework),
                FrameworkId::Sinatra => Box::new(SinatraFramework),
                FrameworkId::ActixWeb => Box::new(ActixFramework),
                FrameworkId::Axum => Box::new(AxumFramework),
                FrameworkId::Gin => Box::new(GinFramework),
                FrameworkId::Echo => Box::new(EchoFramework),
                FrameworkId::AspNetCore => Box::new(AspNetFramework),
                FrameworkId::Laravel => Box::new(LaravelFramework),
                FrameworkId::Symfony => Box::new(SymfonyFramework),
                FrameworkId::Phoenix => Box::new(PhoenixFramework),
                FrameworkId::Custom(_) => continue,
            };
            registry.frameworks.insert(id.clone(), fw);
        }

        registry
            .orchestrators
            .insert(OrchestratorId::Turborepo, Arc::new(TurborepoOrchestrator));
        registry
            .orchestrators
            .insert(OrchestratorId::Nx, Arc::new(NxOrchestrator));
        registry
            .orchestrators
            .insert(OrchestratorId::Lerna, Arc::new(LernaOrchestrator));

        if let Some(llm) = llm_client {
            registry.languages.insert(
                LanguageId::Custom("__llm_fallback__".to_string()),
                Arc::new(LLMLanguage::new(llm.clone())),
            );

            registry.build_systems.insert(
                BuildSystemId::Custom("__llm_fallback__".to_string()),
                Arc::new(LLMBuildSystem::new(llm.clone())),
            );

            registry.frameworks.insert(
                FrameworkId::Custom("__llm_fallback__".to_string()),
                Box::new(LLMFramework::new(llm.clone())),
            );

            registry.orchestrators.insert(
                OrchestratorId::Custom("__llm_fallback__".to_string()),
                Arc::new(LLMOrchestrator::new(llm.clone())),
            );
        }

        registry
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

    pub fn get_orchestrator(&self, id: OrchestratorId) -> Option<&dyn MonorepoOrchestrator> {
        self.orchestrators.get(&id).map(|o| o.as_ref())
    }

    pub fn all_orchestrators(&self) -> Vec<&dyn MonorepoOrchestrator> {
        self.orchestrators.values().map(|o| o.as_ref()).collect()
    }

    pub fn detect_all_stacks(
        &self,
        repo_root: &Path,
        file_tree: &[std::path::PathBuf],
        fs: &dyn crate::fs::FileSystem,
    ) -> anyhow::Result<Vec<DetectionStack>> {
        let mut all_detections = Vec::new();

        for build_system in self.build_systems.values() {
            let detections = build_system.detect_all(repo_root, file_tree, fs)?;
            all_detections.extend(detections);
        }

        Ok(all_detections)
    }


    pub fn all_excluded_dirs(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for lang in self.languages.values() {
            for dir in lang.excluded_dirs() {
                if seen.insert(dir.clone()) {
                    result.push(dir);
                }
            }
        }
        result
    }

    pub fn all_workspace_configs(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for lang in self.languages.values() {
            for config in lang.workspace_configs() {
                if seen.insert(config.clone()) {
                    result.push(config);
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

    pub fn get_runtime(
        &self,
        id: RuntimeId,
        llm_client: Option<Arc<dyn LLMClient>>,
    ) -> Box<dyn crate::stack::runtime::Runtime> {
        match id {
            RuntimeId::JVM => Box::new(crate::stack::runtime::JvmRuntime),
            RuntimeId::Node => Box::new(crate::stack::runtime::NodeRuntime),
            RuntimeId::Python => Box::new(crate::stack::runtime::PythonRuntime),
            RuntimeId::Ruby => Box::new(crate::stack::runtime::RubyRuntime),
            RuntimeId::PHP => Box::new(crate::stack::runtime::PhpRuntime),
            RuntimeId::DotNet => Box::new(crate::stack::runtime::DotNetRuntime),
            RuntimeId::BEAM => Box::new(crate::stack::runtime::BeamRuntime),
            RuntimeId::Native => Box::new(crate::stack::runtime::NativeRuntime),
            RuntimeId::Custom(_) => match llm_client {
                Some(llm) => Box::new(crate::stack::runtime::LLMRuntime::new(llm)),
                None => Box::new(crate::stack::runtime::LLMRuntime::default()),
            },
        }
    }
}

impl Default for StackRegistry {
    fn default() -> Self {
        Self::with_defaults(None)
    }
}
