use super::structure::{MonorepoTool, StructureResult};
use crate::pipeline::Confidence;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCacheInfo {
    pub root_cache_dirs: Vec<PathBuf>,
    pub confidence: Confidence,
}


pub fn execute(structure: &StructureResult) -> RootCacheInfo {
    let root_cache_dirs = match &structure.monorepo_tool {
        Some(MonorepoTool::PnpmWorkspaces) => {
            vec![PathBuf::from("node_modules"), PathBuf::from(".pnpm-store")]
        }
        Some(MonorepoTool::YarnWorkspaces) | Some(MonorepoTool::NpmWorkspaces) => {
            vec![PathBuf::from("node_modules"), PathBuf::from(".yarn/cache")]
        }
        Some(MonorepoTool::CargoWorkspace) => vec![PathBuf::from("target")],
        Some(MonorepoTool::Turborepo) => {
            vec![PathBuf::from("node_modules"), PathBuf::from(".turbo")]
        }
        Some(MonorepoTool::Nx) => vec![PathBuf::from("node_modules"), PathBuf::from(".nx")],
        Some(MonorepoTool::Lerna) => vec![PathBuf::from("node_modules")],
        Some(MonorepoTool::GradleMultiproject) => vec![PathBuf::from(".gradle")],
        Some(MonorepoTool::MavenMultimodule) => vec![PathBuf::from(".m2/repository")],
        Some(MonorepoTool::GoWorkspace) => vec![PathBuf::from("go/pkg/mod")],
        Some(MonorepoTool::Unknown) | None => vec![],
    };

    RootCacheInfo {
        root_cache_dirs,
        confidence: Confidence::High,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::Confidence;
    use crate::pipeline::phases::structure::ProjectType;

    #[test]
    fn test_root_cache_pnpm() {
        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            monorepo_tool: Some(MonorepoTool::PnpmWorkspaces),
            services: vec![],
            packages: vec![],
            confidence: Confidence::High,
        };

        let result = execute(&structure);
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from("node_modules")));
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from(".pnpm-store")));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_root_cache_cargo_workspace() {
        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            monorepo_tool: Some(MonorepoTool::CargoWorkspace),
            services: vec![],
            packages: vec![],
            confidence: Confidence::High,
        };

        let result = execute(&structure);
        assert_eq!(result.root_cache_dirs, vec![PathBuf::from("target")]);
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_root_cache_turborepo() {
        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            monorepo_tool: Some(MonorepoTool::Turborepo),
            services: vec![],
            packages: vec![],
            confidence: Confidence::High,
        };

        let result = execute(&structure);
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from("node_modules")));
        assert!(result.root_cache_dirs.contains(&PathBuf::from(".turbo")));
    }

    #[test]
    fn test_root_cache_none() {
        let structure = StructureResult {
            project_type: ProjectType::SingleService,
            monorepo_tool: None,
            services: vec![],
            packages: vec![],
            confidence: Confidence::High,
        };

        let result = execute(&structure);
        assert!(result.root_cache_dirs.is_empty());
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_root_cache_nx() {
        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            monorepo_tool: Some(MonorepoTool::Nx),
            services: vec![],
            packages: vec![],
            confidence: Confidence::High,
        };

        let result = execute(&structure);
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from("node_modules")));
        assert!(result.root_cache_dirs.contains(&PathBuf::from(".nx")));
    }
}
