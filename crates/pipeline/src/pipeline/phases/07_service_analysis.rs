use super::build::BuildPhase;
use super::cache::CachePhase;
use super::runtime_config::RuntimeConfigPhase;
use super::scan::ScanResult;
use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::{ServicePhase, WorkflowPhase};
use crate::pipeline::service_context::{ServiceContext, Stack};
use anyhow::{Context as AnyhowContext, Result};
use async_trait::async_trait;
use peelbox_stack::detection::DetectionStack;
use peelbox_stack::orchestrator::WorkspaceStructure;
use peelbox_stack::{BuildSystemId, DetectedProject, LanguageId};
use std::path::PathBuf;
use std::sync::Arc;

/// Service definition for analysis
#[derive(Debug, Clone)]
pub struct Service {
    pub path: PathBuf,
    pub manifest: String,
    pub language: LanguageId,
    pub build_system: BuildSystemId,
}

/// A service paired with its pre-computed stack from the Scanner.
struct ServiceWithStack {
    service: Service,
    stack: Option<Stack>,
}

pub struct ServiceAnalysisPhase;

#[async_trait]
impl WorkflowPhase for ServiceAnalysisPhase {
    fn name(&self) -> &'static str {
        "ServiceAnalysisPhase"
    }

    async fn execute(&self, context: &mut AnalysisContext) -> Result<()> {
        let workspace = context
            .workspace
            .as_ref()
            .expect("Workspace must be available before service analysis");

        let scan = context
            .scan
            .as_ref()
            .expect("Scan must be available before service analysis");

        // Convert workspace packages into Service structs by matching with scan detections
        let services = Self::build_services_from_workspace(workspace, scan);

        for sws in &services {
            let analysis_result = self.analyze_service(&sws.service, sws.stack.clone(), context).await;

            match analysis_result {
                Ok(result) => {
                    context.service_analyses.push(result);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to analyze service {}: {}. Skipping service.",
                        sws.service.path.display(),
                        e
                    );
                }
            }
        }

        Ok(())
    }
}

impl ServiceAnalysisPhase {
    /// Build a Service and optional Stack from a detection and its matching DetectedProject.
    fn service_from_detection(
        detection: &DetectionStack,
        project: Option<&DetectedProject>,
        service_path: PathBuf,
    ) -> ServiceWithStack {
        let service = Service {
            path: service_path,
            manifest: detection
                .manifest_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            language: detection.language.clone(),
            build_system: detection.build_system.clone(),
        };

        // Build Stack from DetectedProject if available (Scanner already computed runtime/framework/version)
        let stack = project.map(|p| Stack {
            language: p.language.clone(),
            build_system: p.build_system.clone(),
            framework: p.framework.clone(),
            runtime: p.runtime.clone(),
            version: p.version.clone(),
        });

        ServiceWithStack { service, stack }
    }

    /// Find the DetectedProject matching a given manifest path.
    fn find_project_for_detection<'a>(
        detection: &DetectionStack,
        detected_projects: &'a [DetectedProject],
    ) -> Option<&'a DetectedProject> {
        detected_projects
            .iter()
            .find(|p| p.manifest_path == detection.manifest_path)
    }

    fn build_services_from_workspace(
        workspace: &WorkspaceStructure,
        scan: &ScanResult,
    ) -> Vec<ServiceWithStack> {
        // If workspace has packages, use them (workspace orchestrator detected)
        if !workspace.packages.is_empty() {
            workspace
                .packages
                .iter()
                .filter_map(|package| {
                    // Find detection for this package path
                    scan.detections
                        .iter()
                        .find(|detection| {
                            detection
                                .manifest_path
                                .parent()
                                .unwrap_or_else(|| std::path::Path::new(""))
                                == package.path
                        })
                        .map(|detection| {
                            let project = Self::find_project_for_detection(
                                detection,
                                &scan.detected_projects,
                            );
                            Self::service_from_detection(detection, project, package.path.clone())
                        })
                })
                .collect()
        } else {
            // No workspace orchestrator - build Services directly from scan detections
            scan.detections
                .iter()
                .map(|detection| {
                    let service_path = detection
                        .manifest_path
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .to_path_buf();
                    let project =
                        Self::find_project_for_detection(detection, &scan.detected_projects);
                    Self::service_from_detection(detection, project, service_path)
                })
                .collect()
        }
    }

    async fn analyze_service(
        &self,
        service: &Service,
        stack: Option<Stack>,
        context: &AnalysisContext,
    ) -> Result<ServiceContext> {
        let service_arc = Arc::new(service.clone());
        let context_arc = Arc::new((*context).clone());
        let mut service_context = ServiceContext::new(service_arc, context_arc);

        // Set the pre-computed stack from Scanner (replaces StackIdentificationPhase)
        service_context.stack = stack;

        // Execute remaining service phases in order
        // Note: StackIdentificationPhase is no longer needed — the Scanner already
        // computes runtime, framework, and version during scan.
        let phases: Vec<&dyn ServicePhase> = vec![
            &RuntimeConfigPhase,
            &BuildPhase,
            &CachePhase,
        ];

        for phase in phases {
            tracing::debug!("Executing service phase: {}", phase.name());
            match phase.execute(&mut service_context).await {
                Ok(_) => {
                    tracing::debug!("Service phase {} completed successfully", phase.name());
                }
                Err(e) => {
                    tracing::error!(
                        "Service phase {} failed for service at {}: {:?}",
                        phase.name(),
                        service.path.display(),
                        e
                    );
                    return Err(e).with_context(|| {
                        format!(
                            "{} failed for service at {}",
                            phase.name(),
                            service.path.display()
                        )
                    });
                }
            }
        }

        Ok(service_context)
    }
}
