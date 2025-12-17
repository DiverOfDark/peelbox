pub mod bootstrap;
pub mod cli;
pub mod config;
pub mod detection;
pub mod extractors;
pub mod fs;
pub mod heuristics;
pub mod languages;
pub mod llm;
pub mod output;
pub mod pipeline;
pub mod progress;
pub mod validation;

pub use config::{AipackConfig, ConfigError};
pub use detection::service::{DetectionService, ServiceError};
pub use fs::{FileSystem, MockFileSystem, RealFileSystem};
pub use languages::{LanguageDefinition, LanguageRegistry};
pub use llm::{AdapterKind, BackendError};
pub use llm::{GenAIClient, LLMClient, MockLLMClient, MockResponse};
pub use output::schema::UniversalBuild;
pub use pipeline::{PipelineConfig, PipelineContext};
pub use progress::{LoggingHandler, NoOpHandler, ProgressEvent, ProgressHandler};
pub use validation::{ValidationRule, Validator};

pub fn init_default() {
    use std::sync::Once;
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let filter = EnvFilter::from_default_env()
            .add_directive("aipack=info".parse().unwrap())
            .add_directive("h2=warn".parse().unwrap())
            .add_directive("hyper=warn".parse().unwrap())
            .add_directive("reqwest=warn".parse().unwrap());

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(true))
            .init();
    });
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_name_is_aipack() {
        assert_eq!(NAME, "aipack");
    }
}
