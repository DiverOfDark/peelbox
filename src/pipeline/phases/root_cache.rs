use super::structure::{MonorepoTool, StructureResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCacheInfo {
    pub root_cache_dirs: Vec<PathBuf>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

pub fn execute(structure: &StructureResult) -> RootCacheInfo {
    let root_cache_dirs = match structure.monorepo_tool {
        MonorepoTool::PnpmWorkspaces => vec![
            PathBuf::from("node_modules"),
            PathBuf::from(".pnpm-store"),
        ],
        MonorepoTool::YarnWorkspaces => vec![
            PathBuf::from("node_modules"),
            PathBuf::from(".yarn/cache"),
        ],
        MonorepoTool::CargoWorkspace => vec![PathBuf::from("target")],
        MonorepoTool::Turborepo => vec![
            PathBuf::from("node_modules"),
            PathBuf::from(".turbo"),
        ],
        MonorepoTool::Nx => vec![PathBuf::from("node_modules"), PathBuf::from(".nx")],
        MonorepoTool::Lerna => vec![PathBuf::from("node_modules")],
        MonorepoTool::Gradle => vec![PathBuf::from(".gradle")],
        MonorepoTool::Maven => vec![PathBuf::from(".m2/repository")],
        MonorepoTool::Go => vec![PathBuf::from("go/pkg/mod")],
        MonorepoTool::None => vec![],
    };

    RootCacheInfo {
        root_cache_dirs,
        confidence: Confidence::High,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::phases::structure::ProjectType;

    #[test]
    fn test_root_cache_pnpm() {
        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            monorepo_tool: MonorepoTool::PnpmWorkspaces,
            confidence: crate::pipeline::phases::structure::Confidence::High,
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
            monorepo_tool: MonorepoTool::CargoWorkspace,
            confidence: crate::pipeline::phases::structure::Confidence::High,
        };

        let result = execute(&structure);
        assert_eq!(result.root_cache_dirs, vec![PathBuf::from("target")]);
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_root_cache_turborepo() {
        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            monorepo_tool: MonorepoTool::Turborepo,
            confidence: crate::pipeline::phases::structure::Confidence::High,
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
            monorepo_tool: MonorepoTool::None,
            confidence: crate::pipeline::phases::structure::Confidence::High,
        };

        let result = execute(&structure);
        assert!(result.root_cache_dirs.is_empty());
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_root_cache_nx() {
        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            monorepo_tool: MonorepoTool::Nx,
            confidence: crate::pipeline::phases::structure::Confidence::High,
        };

        let result = execute(&structure);
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from("node_modules")));
        assert!(result.root_cache_dirs.contains(&PathBuf::from(".nx")));
    }
}
