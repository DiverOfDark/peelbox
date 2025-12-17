use aipack::frameworks::FrameworkRegistry;
use aipack::languages::{Dependency, DependencyInfo};
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn test_spring_boot_detection_from_maven() {
    let registry = FrameworkRegistry::new();

    let mut deps_info = DependencyInfo::empty();
    deps_info.external_deps.push(Dependency {
        name: "org.springframework.boot:spring-boot-starter-web".to_string(),
        version: Some("3.2.0".to_string()),
        is_internal: false,
    });

    let result = registry.detect_from_dependencies(&deps_info);
    assert!(result.is_some());

    let (framework, confidence) = result.unwrap();
    assert_eq!(framework.id().name(), "Spring Boot");
    assert_eq!(confidence, 0.95);

    assert_eq!(framework.default_ports(), &[8080]);
    assert_eq!(framework.health_endpoints(), &[
        "/actuator/health",
        "/actuator/health/liveness",
        "/actuator/health/readiness"
    ]);
}

#[test]
fn test_express_detection_from_npm() {
    let registry = FrameworkRegistry::new();

    let mut deps_info = DependencyInfo::empty();
    deps_info.external_deps.push(Dependency {
        name: "express".to_string(),
        version: Some("4.18.0".to_string()),
        is_internal: false,
    });

    let result = registry.detect_from_dependencies(&deps_info);
    assert!(result.is_some());

    let (framework, confidence) = result.unwrap();
    assert_eq!(framework.id().name(), "Express");
    assert_eq!(confidence, 0.95);

    assert_eq!(framework.default_ports(), &[3000]);
    assert_eq!(framework.health_endpoints(), &["/health", "/healthz", "/ping"]);
}

#[test]
fn test_django_detection_from_pip() {
    let registry = FrameworkRegistry::new();

    let mut deps_info = DependencyInfo::empty();
    deps_info.external_deps.push(Dependency {
        name: "django".to_string(),
        version: Some("5.0.0".to_string()),
        is_internal: false,
    });

    let result = registry.detect_from_dependencies(&deps_info);
    assert!(result.is_some());

    let (framework, confidence) = result.unwrap();
    assert_eq!(framework.id().name(), "Django");
    assert_eq!(confidence, 0.95);

    assert_eq!(framework.default_ports(), &[8000]);
    assert_eq!(framework.health_endpoints(), &["/health/", "/healthz/", "/ping/"]);
}

#[test]
fn test_nextjs_detection() {
    let registry = FrameworkRegistry::new();

    let mut deps_info = DependencyInfo::empty();
    deps_info.external_deps.push(Dependency {
        name: "next".to_string(),
        version: Some("14.0.0".to_string()),
        is_internal: false,
    });

    let result = registry.detect_from_dependencies(&deps_info);
    assert!(result.is_some());

    let (framework, confidence) = result.unwrap();
    assert_eq!(framework.id().name(), "Next.js");
    assert_eq!(confidence, 0.95);

    assert_eq!(framework.default_ports(), &[3000]);
}

#[test]
fn test_rails_detection() {
    let registry = FrameworkRegistry::new();

    let mut deps_info = DependencyInfo::empty();
    deps_info.external_deps.push(Dependency {
        name: "rails".to_string(),
        version: Some("7.1.0".to_string()),
        is_internal: false,
    });

    let result = registry.detect_from_dependencies(&deps_info);
    assert!(result.is_some());

    let (framework, confidence) = result.unwrap();
    assert_eq!(framework.id().name(), "Rails");
    assert_eq!(confidence, 0.95);

    assert_eq!(framework.default_ports(), &[3000]);
}

#[test]
fn test_framework_compatibility_validation() {
    let registry = FrameworkRegistry::new();

    let test_cases = vec![
        ("Spring Boot", vec!["Java", "Kotlin"], vec!["maven", "gradle"]),
        ("Express", vec!["JavaScript", "TypeScript"], vec!["npm", "yarn", "pnpm"]),
        ("Django", vec!["Python"], vec!["pip", "poetry"]),
        ("Next.js", vec!["JavaScript", "TypeScript"], vec!["npm", "yarn", "pnpm"]),
        ("Rails", vec!["Ruby"], vec!["bundler"]),
        ("Quarkus", vec!["Java", "Kotlin"], vec!["maven", "gradle"]),
        ("Flask", vec!["Python"], vec!["pip", "poetry"]),
        ("Gin", vec!["Go"], vec!["go"]),
        ("Laravel", vec!["PHP"], vec!["composer"]),
    ];

    for (framework_name, expected_languages, expected_build_systems) in test_cases {
        let framework = registry.get_by_name(framework_name)
            .unwrap_or_else(|| panic!("Framework {} not found in registry", framework_name));

        assert_eq!(framework.id().name(), framework_name);

        for lang in expected_languages {
            assert!(
                framework.compatible_languages().contains(&lang),
                "Framework {} should support language {}",
                framework_name,
                lang
            );
        }

        for build_system in expected_build_systems {
            assert!(
                framework.compatible_build_systems().contains(&build_system),
                "Framework {} should support build system {}",
                framework_name,
                build_system
            );
        }
    }
}

#[test]
fn test_no_framework_detected_for_unknown_dependencies() {
    let registry = FrameworkRegistry::new();

    let mut deps_info = DependencyInfo::empty();
    deps_info.external_deps.push(Dependency {
        name: "some-random-library".to_string(),
        version: Some("1.0.0".to_string()),
        is_internal: false,
    });

    let result = registry.detect_from_dependencies(&deps_info);
    assert!(result.is_none(), "Should not detect framework for unknown dependencies");
}

#[test]
fn test_multiple_frameworks_detection() {
    let registry = FrameworkRegistry::new();

    let mut deps_info = DependencyInfo::empty();
    deps_info.external_deps.push(Dependency {
        name: "next".to_string(),
        version: Some("14.0.0".to_string()),
        is_internal: false,
    });
    deps_info.external_deps.push(Dependency {
        name: "express".to_string(),
        version: Some("4.18.0".to_string()),
        is_internal: false,
    });

    let result = registry.detect_from_dependencies(&deps_info);
    assert!(result.is_some(), "Should detect at least one framework when multiple are present");

    let (framework, _) = result.unwrap();
    assert!(
        framework.id().name() == "Next.js" || framework.id().name() == "Express",
        "Should detect either Next.js or Express"
    );
}

#[test]
fn test_framework_registry_completeness() {
    let registry = FrameworkRegistry::new();

    let expected_frameworks = vec![
        "Spring Boot",
        "Express",
        "Django",
        "Rails",
        "ASP.NET Core",
        "Quarkus",
        "Micronaut",
        "Ktor",
        "Next.js",
        "NestJS",
        "Fastify",
        "Flask",
        "FastAPI",
        "Gin",
        "Laravel",
    ];

    for framework_name in expected_frameworks {
        assert!(
            registry.get_by_name(framework_name).is_some(),
            "Framework {} should be in registry",
            framework_name
        );
    }
}

#[test]
fn test_jvm_framework_detection() {
    let registry = FrameworkRegistry::new();

    let jvm_frameworks = vec![
        ("io.quarkus:quarkus-core", "Quarkus"),
        ("io.micronaut:micronaut-core", "Micronaut"),
        ("io.ktor:ktor-server-core", "Ktor"),
    ];

    for (dependency, expected_framework) in jvm_frameworks {
        let mut deps_info = DependencyInfo::empty();
        deps_info.external_deps.push(Dependency {
            name: dependency.to_string(),
            version: Some("3.0.0".to_string()),
            is_internal: false,
        });

        let result = registry.detect_from_dependencies(&deps_info);
        assert!(result.is_some(), "Should detect {} from {}", expected_framework, dependency);

        let (framework, _) = result.unwrap();
        assert_eq!(framework.id().name(), expected_framework);
    }
}

#[test]
fn test_python_framework_detection() {
    let registry = FrameworkRegistry::new();

    let python_frameworks = vec![
        ("flask", "Flask"),
        ("fastapi", "FastAPI"),
        ("django", "Django"),
    ];

    for (dependency, expected_framework) in python_frameworks {
        let mut deps_info = DependencyInfo::empty();
        deps_info.external_deps.push(Dependency {
            name: dependency.to_string(),
            version: Some("1.0.0".to_string()),
            is_internal: false,
        });

        let result = registry.detect_from_dependencies(&deps_info);
        assert!(result.is_some(), "Should detect {} from {}", expected_framework, dependency);

        let (framework, _) = result.unwrap();
        assert_eq!(framework.id().name(), expected_framework);
    }
}

#[test]
fn test_nodejs_framework_detection() {
    let registry = FrameworkRegistry::new();

    let nodejs_frameworks = vec![
        ("express", "Express"),
        ("next", "Next.js"),
        ("@nestjs/core", "NestJS"),
        ("fastify", "Fastify"),
    ];

    for (dependency, expected_framework) in nodejs_frameworks {
        let mut deps_info = DependencyInfo::empty();
        deps_info.external_deps.push(Dependency {
            name: dependency.to_string(),
            version: Some("1.0.0".to_string()),
            is_internal: false,
        });

        let result = registry.detect_from_dependencies(&deps_info);
        assert!(result.is_some(), "Should detect {} from {}", expected_framework, dependency);

        let (framework, _) = result.unwrap();
        assert_eq!(framework.id().name(), expected_framework);
    }
}

#[test]
fn test_internal_dependencies_ignored() {
    let registry = FrameworkRegistry::new();

    let mut deps_info = DependencyInfo::empty();
    deps_info.internal_deps.push(Dependency {
        name: "express".to_string(),
        version: None,
        is_internal: true,
    });

    let result = registry.detect_from_dependencies(&deps_info);
    assert!(result.is_none(), "Internal dependencies should be ignored for framework detection");
}
