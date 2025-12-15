use super::scan::ScanResult;
use super::structure::Service;
use crate::extractors::context::ServiceContext;

pub fn create_service_context(scan: &ScanResult, service: &Service) -> ServiceContext {
    ServiceContext {
        path: scan.repo_path.join(&service.path),
        language: Some(service.language.clone()),
        build_system: Some(service.build_system.clone()),
    }
}
