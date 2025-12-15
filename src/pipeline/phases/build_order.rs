use super::dependencies::DependencyResult;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildOrderResult {
    pub build_order: Vec<PathBuf>,
    pub has_cycle: bool,
}

pub fn execute(dependencies: &DependencyResult) -> Result<BuildOrderResult> {
    let graph = build_dependency_graph(dependencies);

    let (order, has_cycle) = topological_sort(&graph);

    Ok(BuildOrderResult {
        build_order: order,
        has_cycle,
    })
}

fn build_dependency_graph(
    dependencies: &DependencyResult,
) -> HashMap<PathBuf, Vec<PathBuf>> {
    let mut graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    for (path, dep_info) in &dependencies.dependencies {
        graph
            .entry(path.clone())
            .or_default()
            .extend(dep_info.internal_deps.clone());
    }

    graph
}

fn topological_sort(graph: &HashMap<PathBuf, Vec<PathBuf>>) -> (Vec<PathBuf>, bool) {
    let mut in_degree: HashMap<PathBuf, usize> = HashMap::new();
    let mut nodes: HashSet<PathBuf> = HashSet::new();

    for (node, deps) in graph {
        nodes.insert(node.clone());
        in_degree.entry(node.clone()).or_insert(0);

        for dep in deps {
            nodes.insert(dep.clone());
            *in_degree.entry(dep.clone()).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<PathBuf> = in_degree
        .iter()
        .filter(|(_, &degree)| degree == 0)
        .map(|(node, _)| node.clone())
        .collect();

    let mut result = Vec::new();
    let mut visited = 0;

    while let Some(node) = queue.pop_front() {
        result.push(node.clone());
        visited += 1;

        if let Some(dependents) = graph.get(&node) {
            for dependent in dependents {
                if let Some(degree) = in_degree.get_mut(dependent) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }
    }

    let has_cycle = visited < nodes.len();

    if has_cycle {
        let remaining: Vec<PathBuf> = nodes
            .into_iter()
            .filter(|n| !result.contains(n))
            .collect();
        result.extend(remaining);
    }

    (result, has_cycle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::{Confidence, DependencyInfo, DetectionMethod};

    #[test]
    fn test_simple_linear_dependencies() {
        let mut deps = HashMap::new();

        deps.insert(
            PathBuf::from("app"),
            DependencyInfo {
                path: PathBuf::from("app"),
                internal_deps: vec![PathBuf::from("lib")],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
                confidence: Confidence::High,
            },
        );

        deps.insert(
            PathBuf::from("lib"),
            DependencyInfo {
                path: PathBuf::from("lib"),
                internal_deps: vec![],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
                confidence: Confidence::High,
            },
        );

        let result = execute(&DependencyResult { dependencies: deps }).unwrap();

        assert!(!result.has_cycle);
        assert_eq!(result.build_order.len(), 2);

        let lib_idx = result.build_order.iter().position(|p| p == &PathBuf::from("lib")).unwrap();
        let app_idx = result.build_order.iter().position(|p| p == &PathBuf::from("app")).unwrap();

        assert!(lib_idx < app_idx);
    }

    #[test]
    fn test_diamond_dependencies() {
        let mut deps = HashMap::new();

        deps.insert(
            PathBuf::from("app"),
            DependencyInfo {
                path: PathBuf::from("app"),
                internal_deps: vec![PathBuf::from("lib1"), PathBuf::from("lib2")],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
                confidence: Confidence::High,
            },
        );

        deps.insert(
            PathBuf::from("lib1"),
            DependencyInfo {
                path: PathBuf::from("lib1"),
                internal_deps: vec![PathBuf::from("base")],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
                confidence: Confidence::High,
            },
        );

        deps.insert(
            PathBuf::from("lib2"),
            DependencyInfo {
                path: PathBuf::from("lib2"),
                internal_deps: vec![PathBuf::from("base")],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
                confidence: Confidence::High,
            },
        );

        deps.insert(
            PathBuf::from("base"),
            DependencyInfo {
                path: PathBuf::from("base"),
                internal_deps: vec![],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
                confidence: Confidence::High,
            },
        );

        let result = execute(&DependencyResult { dependencies: deps }).unwrap();

        assert!(!result.has_cycle);
        assert_eq!(result.build_order.len(), 4);

        let base_idx = result.build_order.iter().position(|p| p == &PathBuf::from("base")).unwrap();
        let lib1_idx = result.build_order.iter().position(|p| p == &PathBuf::from("lib1")).unwrap();
        let lib2_idx = result.build_order.iter().position(|p| p == &PathBuf::from("lib2")).unwrap();
        let app_idx = result.build_order.iter().position(|p| p == &PathBuf::from("app")).unwrap();

        assert!(base_idx < lib1_idx);
        assert!(base_idx < lib2_idx);
        assert!(lib1_idx < app_idx);
        assert!(lib2_idx < app_idx);
    }

    #[test]
    fn test_cycle_detection() {
        let mut deps = HashMap::new();

        deps.insert(
            PathBuf::from("app1"),
            DependencyInfo {
                path: PathBuf::from("app1"),
                internal_deps: vec![PathBuf::from("app2")],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
                confidence: Confidence::High,
            },
        );

        deps.insert(
            PathBuf::from("app2"),
            DependencyInfo {
                path: PathBuf::from("app2"),
                internal_deps: vec![PathBuf::from("app1")],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
                confidence: Confidence::High,
            },
        );

        let result = execute(&DependencyResult { dependencies: deps }).unwrap();

        assert!(result.has_cycle);
        assert_eq!(result.build_order.len(), 2);
    }

    #[test]
    fn test_no_dependencies() {
        let mut deps = HashMap::new();

        deps.insert(
            PathBuf::from("app1"),
            DependencyInfo {
                path: PathBuf::from("app1"),
                internal_deps: vec![],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
                confidence: Confidence::High,
            },
        );

        deps.insert(
            PathBuf::from("app2"),
            DependencyInfo {
                path: PathBuf::from("app2"),
                internal_deps: vec![],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
                confidence: Confidence::High,
            },
        );

        let result = execute(&DependencyResult { dependencies: deps }).unwrap();

        assert!(!result.has_cycle);
        assert_eq!(result.build_order.len(), 2);
    }
}
