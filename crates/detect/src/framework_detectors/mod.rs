//! Framework detectors.
//!
//! Each detector checks dependencies for framework indicators and returns a `FrameworkContribution`.

mod dotnet;
mod elixir;
mod go_fw;
mod jvm;
mod nodejs;
mod php;
mod python;
mod ruby;
mod rust_fw;
mod zig_fw;

pub use dotnet::*;
pub use elixir::*;
pub use go_fw::*;
pub use jvm::*;
pub use nodejs::*;
pub use php::*;
pub use python::*;
pub use ruby::*;
pub use rust_fw::*;
pub use zig_fw::*;

macro_rules! simple_detector {
    ($name:ident, $id:expr, $langs:expr, $detect_fn:expr, $ports:expr, $health:expr, $env:expr, $pkgs:expr) => {
        pub struct $name;
        impl $crate::traits::FrameworkDetector for $name {
            fn id(&self) -> $crate::ids::FrameworkId {
                $id
            }
            fn compatible_languages(&self) -> &[$crate::ids::LanguageId] {
                $langs
            }
            fn detect(&self, deps: &[$crate::types::Dependency]) -> bool {
                ($detect_fn)(deps)
            }
            fn contribution(
                &self,
                _deps: &[$crate::types::Dependency],
            ) -> $crate::types::FrameworkContribution {
                $crate::types::FrameworkContribution {
                    framework: $id,
                    default_ports: $ports,
                    health_endpoints: $health,
                    env_vars: $env,
                    runtime_packages: $pkgs,
                    runtime_command: None,
                    runtime_env: ::std::collections::BTreeMap::new(),
                    workdir: None,
                    extra_copy: vec![],
                }
            }
        }
    };
}

// Re-export macro for use in submodules
pub(crate) use simple_detector;

#[cfg(test)]
mod tests {
    use crate::ids::FrameworkId;
    use crate::traits::FrameworkDetector;
    use crate::types::DepScope;
    use crate::types::Dependency;

    fn make_dep(name: &str) -> Dependency {
        Dependency {
            name: name.to_string(),
            version: None,
            scope: DepScope::Runtime,
            is_internal: false,
        }
    }

    const SPRING_BOOT: FrameworkId = FrameworkId::new("spring-boot");

    #[test]
    fn test_spring_boot_detection() {
        let detector = super::SpringBootDetector;
        let deps = vec![make_dep("org.springframework.boot:spring-boot-starter-web")];
        assert!(detector.detect(&deps));
        let contrib = detector.contribution(&deps);
        assert_eq!(contrib.framework, SPRING_BOOT);
        assert_eq!(contrib.default_ports, vec![8080]);
    }

    #[test]
    fn test_express_detection() {
        let detector = super::ExpressDetector;
        let deps = vec![make_dep("express"), make_dep("body-parser")];
        assert!(detector.detect(&deps));
    }

    #[test]
    fn test_no_false_positive() {
        let detector = super::SpringBootDetector;
        let deps = vec![make_dep("express"), make_dep("react")];
        assert!(!detector.detect(&deps));
    }

    #[test]
    fn test_django_detection() {
        let detector = super::DjangoDetector;
        let deps = vec![make_dep("django")];
        assert!(detector.detect(&deps));
    }

    #[test]
    fn test_gin_detection() {
        let detector = super::GinDetector;
        let deps = vec![make_dep("github.com/gin-gonic/gin")];
        assert!(detector.detect(&deps));
    }
}
