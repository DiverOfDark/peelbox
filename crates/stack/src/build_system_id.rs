crate::define_id_enum! {
    /// Build system identifier with support for LLM-discovered build systems
    BuildSystemId {
        Cargo => "cargo" : "Cargo" | "cargo",
        Maven => "maven" : "Maven" | "maven",
        Gradle => "gradle" : "Gradle" | "gradle",
        Npm => "npm" : "npm",
        Yarn => "yarn" : "Yarn" | "yarn",
        Pnpm => "pnpm" : "pnpm",
        Bun => "bun" : "Bun" | "bun",
        Pip => "pip" : "pip",
        Poetry => "poetry" : "Poetry" | "poetry",
        Pipenv => "pipenv" : "Pipenv" | "pipenv",
        GoMod => "go-mod" : "go mod" | "go-mod",
        DotNet => "dotnet" : ".NET" | "dotnet",
        Composer => "composer" : "Composer" | "composer",
        Bundler => "bundler" : "Bundler" | "bundler",
        CMake => "cmake" : "CMake" | "cmake",
        Make => "make" : "Make" | "make",
        Meson => "meson" : "Meson" | "meson",
        Mix => "mix" : "Mix" | "mix",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_system_id_serialization() {
        assert_eq!(
            serde_json::to_string(&BuildSystemId::Npm).unwrap(),
            "\"npm\""
        );
        assert_eq!(
            serde_json::to_string(&BuildSystemId::GoMod).unwrap(),
            "\"go-mod\""
        );
    }

    #[test]
    fn test_build_system_id_name() {
        assert_eq!(BuildSystemId::Cargo.name(), "Cargo");
        assert_eq!(BuildSystemId::GoMod.name(), "go mod");
    }

    #[test]
    fn test_custom_build_system_serialization() {
        let custom = BuildSystemId::Custom("Bazel".to_string());
        assert_eq!(serde_json::to_string(&custom).unwrap(), "\"Bazel\"");
    }

    #[test]
    fn test_custom_build_system_deserialization() {
        let deserialized: BuildSystemId = serde_json::from_str("\"bazel\"").unwrap();
        assert_eq!(deserialized, BuildSystemId::Custom("bazel".to_string()));
        assert_eq!(deserialized.name(), "bazel");
    }

    #[test]
    fn test_from_name_with_aliases() {
        assert_eq!(
            BuildSystemId::from_name("Cargo"),
            Some(BuildSystemId::Cargo)
        );
        assert_eq!(
            BuildSystemId::from_name("cargo"),
            Some(BuildSystemId::Cargo)
        );
        assert_eq!(
            BuildSystemId::from_name("go mod"),
            Some(BuildSystemId::GoMod)
        );
        assert_eq!(
            BuildSystemId::from_name("go-mod"),
            Some(BuildSystemId::GoMod)
        );
        assert_eq!(
            BuildSystemId::from_name(".NET"),
            Some(BuildSystemId::DotNet)
        );
        assert_eq!(
            BuildSystemId::from_name("dotnet"),
            Some(BuildSystemId::DotNet)
        );
        assert_eq!(BuildSystemId::from_name("unknown"), None);
    }
}
