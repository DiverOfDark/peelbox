//! Newtype ID definitions for languages, build systems, frameworks, runtimes, and orchestrators.
//!
//! Each ID is a thin `&'static str` wrapper with `Copy` semantics. Constants are defined
//! in the parser/detector files that own them (not centrally), and metadata is registered
//! via `inventory` for display name lookup.

macro_rules! define_id_type {
    ($name:ident, $meta:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(pub(crate) &'static str);

        pub struct $meta {
            pub slug: &'static str,
            pub display_name: &'static str,
            pub aliases: &'static [&'static str],
        }

        inventory::collect!($meta);

        impl $name {
            pub const fn new(slug: &'static str) -> Self {
                Self(slug)
            }

            pub fn slug(&self) -> &'static str {
                self.0
            }

            pub fn name(&self) -> String {
                for meta in inventory::iter::<$meta> {
                    if meta.slug == self.0 {
                        return meta.display_name.to_string();
                    }
                }
                self.0.to_string()
            }

            pub fn from_name(name: &str) -> Option<Self> {
                for meta in inventory::iter::<$meta> {
                    if meta.display_name == name {
                        return Some(Self(meta.slug));
                    }
                    for alias in meta.aliases {
                        if *alias == name {
                            return Some(Self(meta.slug));
                        }
                    }
                }
                None
            }

            pub fn all_variants() -> Vec<Self> {
                inventory::iter::<$meta>
                    .into_iter()
                    .map(|m| Self(m.slug))
                    .collect()
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(self.0)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                for meta in inventory::iter::<$meta> {
                    if meta.slug == s.as_str() {
                        return Ok(Self(meta.slug));
                    }
                }
                // Unknown slug: leak the string to get 'static lifetime
                Ok(Self(Box::leak(s.into_boxed_str())))
            }
        }
    };
}

// ── Language ID ─────────────────────────────────────────────────────────────

define_id_type!(LanguageId, LanguageMeta);

// ── Build System ID ─────────────────────────────────────────────────────────

define_id_type!(BuildSystemId, BuildSystemMeta);

// ── Framework ID ────────────────────────────────────────────────────────────

define_id_type!(FrameworkId, FrameworkMeta);

// ── Runtime ID ──────────────────────────────────────────────────────────────

define_id_type!(RuntimeId, RuntimeMeta);

impl std::fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ── Orchestrator ID ─────────────────────────────────────────────────────────

define_id_type!(OrchestratorId, OrchestratorMeta);

#[cfg(test)]
mod tests {
    use super::*;

    const RUST: LanguageId = LanguageId::new("rust");
    const CSHARP: LanguageId = LanguageId::new("csharp");
    const C: LanguageId = LanguageId::new("c");
    const CPP: LanguageId = LanguageId::new("c++");

    // ── LanguageId tests ────────────────────────────────────────────────

    #[test]
    fn test_language_id_serialization() {
        assert_eq!(serde_json::to_string(&RUST).unwrap(), "\"rust\"");
        assert_eq!(serde_json::to_string(&CSHARP).unwrap(), "\"csharp\"");
        assert_eq!(serde_json::to_string(&C).unwrap(), "\"c\"");
        assert_eq!(serde_json::to_string(&CPP).unwrap(), "\"c++\"");
    }

    #[test]
    fn test_language_id_deserialization() {
        assert_eq!(
            serde_json::from_str::<LanguageId>("\"rust\"").unwrap(),
            RUST
        );
        assert_eq!(
            serde_json::from_str::<LanguageId>("\"csharp\"").unwrap(),
            CSHARP
        );
    }

    #[test]
    fn test_language_id_name() {
        assert_eq!(RUST.name(), "Rust");
        assert_eq!(CSHARP.name(), "C#");
        assert_eq!(LanguageId::new("fsharp").name(), "F#");
        assert_eq!(C.name(), "C");
        assert_eq!(CPP.name(), "C++");
    }

    #[test]
    fn test_unknown_language_serialization() {
        let unknown = LanguageId::new("UnknownLang");
        assert_eq!(serde_json::to_string(&unknown).unwrap(), "\"UnknownLang\"");
    }

    #[test]
    fn test_unknown_language_deserialization() {
        let deserialized: LanguageId = serde_json::from_str("\"UnknownLang\"").unwrap();
        assert_eq!(deserialized.name(), "UnknownLang");
    }

    // ── BuildSystemId tests ─────────────────────────────────────────────

    #[test]
    fn test_build_system_id_serialization() {
        let npm = BuildSystemId::new("npm");
        let go_mod = BuildSystemId::new("go-mod");
        assert_eq!(serde_json::to_string(&npm).unwrap(), "\"npm\"");
        assert_eq!(serde_json::to_string(&go_mod).unwrap(), "\"go-mod\"");
    }

    #[test]
    fn test_build_system_id_name() {
        assert_eq!(BuildSystemId::new("cargo").name(), "Cargo");
        assert_eq!(BuildSystemId::new("go-mod").name(), "go mod");
    }

    #[test]
    fn test_bazel_build_system_serialization() {
        let bazel = BuildSystemId::new("bazel");
        assert_eq!(serde_json::to_string(&bazel).unwrap(), "\"bazel\"");
    }

    #[test]
    fn test_bazel_build_system_deserialization() {
        let deserialized: BuildSystemId = serde_json::from_str("\"bazel\"").unwrap();
        assert_eq!(deserialized, BuildSystemId::new("bazel"));
        assert_eq!(deserialized.name(), "Bazel");
    }

    #[test]
    fn test_build_system_from_name_with_aliases() {
        assert_eq!(
            BuildSystemId::from_name("Cargo"),
            Some(BuildSystemId::new("cargo"))
        );
        assert_eq!(
            BuildSystemId::from_name("cargo"),
            Some(BuildSystemId::new("cargo"))
        );
        assert_eq!(
            BuildSystemId::from_name("go mod"),
            Some(BuildSystemId::new("go-mod"))
        );
        assert_eq!(
            BuildSystemId::from_name("go-mod"),
            Some(BuildSystemId::new("go-mod"))
        );
        assert_eq!(
            BuildSystemId::from_name(".NET"),
            Some(BuildSystemId::new("dotnet"))
        );
        assert_eq!(
            BuildSystemId::from_name("dotnet"),
            Some(BuildSystemId::new("dotnet"))
        );
        assert_eq!(BuildSystemId::from_name("unknown"), None);
    }

    // ── FrameworkId tests ───────────────────────────────────────────────

    #[test]
    fn test_framework_id_serialization() {
        let spring_boot = FrameworkId::new("spring-boot");
        let nextjs = FrameworkId::new("nextjs");
        assert_eq!(
            serde_json::to_string(&spring_boot).unwrap(),
            "\"spring-boot\""
        );
        assert_eq!(serde_json::to_string(&nextjs).unwrap(), "\"nextjs\"");
    }

    #[test]
    fn test_framework_id_name() {
        assert_eq!(FrameworkId::new("spring-boot").name(), "Spring Boot");
        assert_eq!(FrameworkId::new("nextjs").name(), "Next.js");
    }

    #[test]
    fn test_unknown_framework_serialization() {
        let unknown = FrameworkId::new("unknown-fw");
        assert_eq!(serde_json::to_string(&unknown).unwrap(), "\"unknown-fw\"");
    }

    #[test]
    fn test_unknown_framework_deserialization() {
        let deserialized: FrameworkId = serde_json::from_str("\"qwik\"").unwrap();
        assert_eq!(deserialized.name(), "qwik");
    }

    // ── RuntimeId tests ─────────────────────────────────────────────────

    #[test]
    fn test_runtime_from_name_with_aliases() {
        assert_eq!(RuntimeId::from_name("JVM"), Some(RuntimeId::new("jvm")));
        assert_eq!(RuntimeId::from_name("java"), Some(RuntimeId::new("jvm")));
        assert_eq!(RuntimeId::from_name("kotlin"), Some(RuntimeId::new("jvm")));
        assert_eq!(
            RuntimeId::from_name("Native"),
            Some(RuntimeId::new("native"))
        );
        assert_eq!(RuntimeId::from_name("rust"), Some(RuntimeId::new("native")));
        assert_eq!(RuntimeId::from_name("c"), Some(RuntimeId::new("native")));
        assert_eq!(RuntimeId::from_name("c++"), Some(RuntimeId::new("native")));
        assert_eq!(RuntimeId::from_name("go"), Some(RuntimeId::new("native")));
        assert_eq!(RuntimeId::from_name(".NET"), Some(RuntimeId::new("dotnet")));
        assert_eq!(
            RuntimeId::from_name("dotnet"),
            Some(RuntimeId::new("dotnet"))
        );
        assert_eq!(
            RuntimeId::from_name("csharp"),
            Some(RuntimeId::new("dotnet"))
        );
        assert_eq!(
            RuntimeId::from_name("fsharp"),
            Some(RuntimeId::new("dotnet"))
        );
        assert_eq!(RuntimeId::from_name("unknown"), None);
    }

    #[test]
    fn test_runtime_display() {
        assert_eq!(format!("{}", RuntimeId::new("jvm")), "JVM");
        assert_eq!(format!("{}", RuntimeId::new("node")), "Node");
        assert_eq!(format!("{}", RuntimeId::new("dotnet")), ".NET");
    }

    #[test]
    fn test_unknown_runtime_display() {
        let unknown = RuntimeId::new("unknown-rt");
        assert_eq!(format!("{}", unknown), "unknown-rt");
    }
}
