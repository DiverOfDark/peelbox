use super::scan::ScanResult;
use super::service_analysis::Service;
use crate::extractors::context::ServiceContext;

pub fn create_service_context(scan: &ScanResult, service: &Service) -> ServiceContext {
    ServiceContext {
        path: scan.repo_path.join(&service.path),
        language: Some(service.language),
        build_system: Some(service.build_system),
    }
}
