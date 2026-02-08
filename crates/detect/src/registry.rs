//! Parser and detector registry.

use crate::traits::{ConfigParser, FrameworkDetector, ManifestParser};

/// Central registry of all parsers and detectors.
pub struct Registry {
    pub manifest_parsers: Vec<Box<dyn ManifestParser>>,
    pub config_parsers: Vec<Box<dyn ConfigParser>>,
    pub framework_detectors: Vec<Box<dyn FrameworkDetector>>,
}

impl Registry {
    /// Build a registry with all built-in parsers and detectors.
    pub fn with_defaults() -> Self {
        use crate::framework_detectors::*;
        use crate::parsers::config::*;
        use crate::parsers::manifest::*;

        Self {
            manifest_parsers: vec![
                Box::new(CargoTomlParser),
                // Lockfile parsers BEFORE PackageJsonParser so they take precedence
                Box::new(YarnLockParser),
                Box::new(PnpmLockParser),
                Box::new(PackageJsonParser),
                Box::new(PomXmlParser),
                Box::new(BuildGradleParser),
                Box::new(SettingsGradleParser),
                Box::new(GoModParser),
                Box::new(PyProjectTomlParser),
                Box::new(RequirementsTxtParser),
                Box::new(SetupPyParser),
                Box::new(GemfileParser),
                Box::new(CsprojParser),
                Box::new(MixExsParser),
                Box::new(ComposerJsonParser),
                Box::new(CMakeListsParser),
                Box::new(MakefileParser),
                Box::new(MesonBuildParser),
                Box::new(BuildZigZonParser),
                Box::new(ZigBuildParser),
                Box::new(DenoJsonParser),
                Box::new(BazelBuildParser),
            ],
            config_parsers: vec![
                Box::new(EnvFileParser),
                Box::new(DockerComposeParser),
                Box::new(DockerfileParser),
            ],
            framework_detectors: vec![
                Box::new(SpringBootDetector),
                Box::new(QuarkusDetector),
                Box::new(MicronautDetector),
                Box::new(KtorDetector),
                Box::new(ExpressDetector),
                Box::new(NextJsDetector),
                Box::new(NestJsDetector),
                Box::new(FastifyDetector),
                Box::new(DjangoDetector),
                Box::new(FlaskDetector),
                Box::new(FastApiDetector),
                Box::new(RailsDetector),
                Box::new(SinatraDetector),
                Box::new(ActixWebDetector),
                Box::new(AxumDetector),
                Box::new(GinDetector),
                Box::new(EchoDetector),
                Box::new(AspNetCoreDetector),
                Box::new(LaravelDetector),
                Box::new(SymfonyDetector),
                Box::new(PhoenixDetector),
                Box::new(ZapDetector),
            ],
        }
    }
}
