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

fn build_dependency_graph(dependencies: &DependencyResult) -> HashMap<PathBuf, Vec<PathBuf>> {
    let mut graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    for (path, dep_info) in &dependencies.dependencies {
        let internal_paths: Vec<PathBuf> = dep_info
            .internal_deps
            .iter()
            .map(|dep| PathBuf::from(&dep.name))
            .collect();

        graph
            .entry(path.clone())
            .or_default()
            .extend(internal_paths);
    }

    graph
}

fn topological_sort(graph: &HashMap<PathBuf, Vec<PathBuf>>) -> (Vec<PathBuf>, bool) {
    let mut in_degree: HashMap<PathBuf, usize> = HashMap::new();
    let mut reverse_graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let mut nodes: HashSet<PathBuf> = HashSet::new();

    for (node, deps) in graph {
        nodes.insert(node.clone());
        in_degree.entry(node.clone()).or_insert(0);

        for dep in deps {
            nodes.insert(dep.clone());
            in_degree.entry(dep.clone()).or_insert(0);

            reverse_graph
                .entry(dep.clone())
                .or_default()
                .push(node.clone());

            *in_degree.get_mut(node).unwrap() += 1;
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

        if let Some(dependents) = reverse_graph.get(&node) {
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
        let remaining: Vec<PathBuf> = nodes.into_iter().filter(|n| !result.contains(n)).collect();
        result.extend(remaining);
    }

    (result, has_cycle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::{Dependency, DependencyInfo, DetectionMethod};

    #[test]
    fn test_simple_linear_dependencies() {
        let mut deps = HashMap::new();

        deps.insert(
            PathBuf::from("app"),
            DependencyInfo {
                internal_deps: vec![Dependency {
                    name: "lib".to_string(),
                    version: None,
                    is_internal: true,
                }],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
            },
        );

        deps.insert(
            PathBuf::from("lib"),
            DependencyInfo {
                internal_deps: vec![],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
            },
        );

        let result = execute(&DependencyResult { dependencies: deps }).unwrap();

        assert!(!result.has_cycle);
        assert_eq!(result.build_order.len(), 2);

        let lib_idx = result
            .build_order
            .iter()
            .position(|p| p == &PathBuf::from("lib"))
            .unwrap();
        let app_idx = result
            .build_order
            .iter()
            .position(|p| p == &PathBuf::from("app"))
            .unwrap();

        assert!(lib_idx < app_idx);
    }

    #[test]
    fn test_diamond_dependencies() {
        let mut deps = HashMap::new();

        deps.insert(
            PathBuf::from("app"),
            DependencyInfo {
                internal_deps: vec![
                    Dependency {
                        name: "lib1".to_string(),
                        version: None,
                        is_internal: true,
                    },
                    Dependency {
                        name: "lib2".to_string(),
                        version: None,
                        is_internal: true,
                    },
                ],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
            },
        );

        deps.insert(
            PathBuf::from("lib1"),
            DependencyInfo {
                internal_deps: vec![Dependency {
                    name: "base".to_string(),
                    version: None,
                    is_internal: true,
                }],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
            },
        );

        deps.insert(
            PathBuf::from("lib2"),
            DependencyInfo {
                internal_deps: vec![Dependency {
                    name: "base".to_string(),
                    version: None,
                    is_internal: true,
                }],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
            },
        );

        deps.insert(
            PathBuf::from("base"),
            DependencyInfo {
                internal_deps: vec![],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
            },
        );

        let result = execute(&DependencyResult { dependencies: deps }).unwrap();

        assert!(!result.has_cycle);
        assert_eq!(result.build_order.len(), 4);

        let base_idx = result
            .build_order
            .iter()
            .position(|p| p == &PathBuf::from("base"))
            .unwrap();
        let lib1_idx = result
            .build_order
            .iter()
            .position(|p| p == &PathBuf::from("lib1"))
            .unwrap();
        let lib2_idx = result
            .build_order
            .iter()
            .position(|p| p == &PathBuf::from("lib2"))
            .unwrap();
        let app_idx = result
            .build_order
            .iter()
            .position(|p| p == &PathBuf::from("app"))
            .unwrap();

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
                internal_deps: vec![Dependency {
                    name: "app2".to_string(),
                    version: None,
                    is_internal: true,
                }],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
            },
        );

        deps.insert(
            PathBuf::from("app2"),
            DependencyInfo {
                internal_deps: vec![Dependency {
                    name: "app1".to_string(),
                    version: None,
                    is_internal: true,
                }],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
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
                internal_deps: vec![],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
            },
        );

        deps.insert(
            PathBuf::from("app2"),
            DependencyInfo {
                internal_deps: vec![],
                external_deps: vec![],
                detected_by: DetectionMethod::Deterministic,
            },
        );

        let result = execute(&DependencyResult { dependencies: deps }).unwrap();

        assert!(!result.has_cycle);
        assert_eq!(result.build_order.len(), 2);
    }
}
