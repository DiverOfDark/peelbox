use super::scan::ScanResult;
use super::structure::Service;
use crate::extractors::context::ServiceContext;
use crate::fs::RealFileSystem;
use crate::languages::LanguageRegistry;

pub fn create_service_context(scan: &ScanResult, service: &Service) -> ServiceContext {
    ServiceContext {
        path: scan.repo_path.join(&service.path),
        language: Some(service.language.clone()),
        build_system: Some(service.build_system.clone()),
    }
}

pub fn create_extractor_components() -> (RealFileSystem, LanguageRegistry) {
    (RealFileSystem, LanguageRegistry::with_defaults())
}
