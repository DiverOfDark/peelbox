pub mod cli;
pub mod config;
pub mod detection;
pub mod extractors;
pub mod fs;
pub mod heuristics;
pub mod llm;
pub mod output;
pub mod pipeline;
pub mod progress;
pub mod runtime;
pub mod stack;
pub mod validation;

pub use config::{AipackConfig, ConfigError};
pub use detection::service::{DetectionService, ServiceError};
pub use fs::{FileSystem, MockFileSystem, RealFileSystem};
pub use llm::{AdapterKind, BackendError};
pub use llm::{GenAIClient, LLMClient, MockLLMClient, MockResponse};
pub use output::schema::UniversalBuild;
pub use progress::{LoggingHandler, ProgressEvent};
pub use runtime::{HealthCheck, Runtime, RuntimeConfig};
pub use stack::buildsystem::{BuildSystem, BuildTemplate};
pub use stack::detection::DetectionStack;
pub use stack::framework::Framework;
pub use stack::language::LanguageDefinition;
pub use stack::registry::StackRegistry;
pub use stack::{BuildSystemId, FrameworkId, LanguageId};
pub use validation::Validator;

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
    fn test_name_is_aipack() {
        assert_eq!(NAME, "aipack");
    }
}
