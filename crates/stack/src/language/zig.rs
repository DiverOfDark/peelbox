//! Zig language definition

use super::{DetectionResult, LanguageDefinition};

pub struct ZigLanguage;

impl LanguageDefinition for ZigLanguage {
    fn id(&self) -> crate::LanguageId {
        crate::LanguageId::Custom("Zig".to_string())
    }

    fn extensions(&self) -> Vec<String> {
        vec!["zig".to_string()]
    }

    fn detect(
        &self,
        manifest_name: &str,
        _manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        match manifest_name {
            "build.zig" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Custom("Zig Build".to_string()),
                confidence: 1.0,
            }),
            _ => None,
        }
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["zig build".to_string()]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec!["zig-cache".to_string(), "zig-out".to_string()]
    }
}
