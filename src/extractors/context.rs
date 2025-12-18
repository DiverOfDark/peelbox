//! Service context for extractor operations

use std::path::PathBuf;
use crate::stack::{BuildSystemId, LanguageId};

/// Context for service-level analysis
///
/// Extractors operate at the service level (not repository level).
/// For monorepos, each service within the monorepo gets its own ServiceContext.
#[derive(Debug, Clone)]
pub struct ServiceContext {
    /// Service root path (e.g., "." for single projects or "packages/web" for monorepos)
    pub path: PathBuf,

    /// Detected language for this service (from bootstrap phase)
    pub language: Option<LanguageId>,

    /// Detected build system
    pub build_system: Option<BuildSystemId>,
}

impl ServiceContext {
    /// Create a new ServiceContext
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            language: None,
            build_system: None,
        }
    }

    /// Create ServiceContext with language and build system
    pub fn with_detection(
        path: PathBuf,
        language: Option<LanguageId>,
        build_system: Option<BuildSystemId>,
    ) -> Self {
        Self {
            path,
            language,
            build_system,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let ctx = ServiceContext::new(PathBuf::from("/path/to/service"));
        assert_eq!(ctx.path, PathBuf::from("/path/to/service"));
        assert!(ctx.language.is_none());
        assert!(ctx.build_system.is_none());
    }

    #[test]
    fn test_with_detection() {
        let ctx = ServiceContext::with_detection(
            PathBuf::from("/path/to/service"),
            Some(LanguageId::Rust),
            Some(BuildSystemId::Cargo),
        );
        assert_eq!(ctx.path, PathBuf::from("/path/to/service"));
        assert_eq!(ctx.language, Some(LanguageId::Rust));
        assert_eq!(ctx.build_system, Some(BuildSystemId::Cargo));
    }
}
