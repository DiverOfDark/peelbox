crate::define_id_enum_with_display! {
    /// Runtime identifier with support for LLM-discovered runtimes
    RuntimeId {
        JVM => "jvm" : "JVM" | "java" | "kotlin",
        Node => "node" : "Node" | "node",
        Python => "python" : "Python" | "python",
        Ruby => "ruby" : "Ruby" | "ruby",
        PHP => "php" : "PHP" | "php",
        DotNet => "dotnet" : ".NET" | "dotnet" | "csharp" | "fsharp",
        BEAM => "beam" : "BEAM" | "elixir",
        Native => "native" : "Native" | "rust" | "c++" | "go",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_runtime_serialization() {
        let custom = RuntimeId::Custom("Deno".to_string());
        assert_eq!(serde_json::to_string(&custom).unwrap(), "\"Deno\"");
    }

    #[test]
    fn test_custom_runtime_deserialization() {
        let deserialized: RuntimeId = serde_json::from_str("\"bun\"").unwrap();
        assert_eq!(deserialized, RuntimeId::Custom("bun".to_string()));
        assert_eq!(deserialized.name(), "bun");
    }

    #[test]
    fn test_from_name_with_aliases() {
        assert_eq!(RuntimeId::from_name("JVM"), Some(RuntimeId::JVM));
        assert_eq!(RuntimeId::from_name("java"), Some(RuntimeId::JVM));
        assert_eq!(RuntimeId::from_name("kotlin"), Some(RuntimeId::JVM));
        assert_eq!(RuntimeId::from_name("Native"), Some(RuntimeId::Native));
        assert_eq!(RuntimeId::from_name("rust"), Some(RuntimeId::Native));
        assert_eq!(RuntimeId::from_name("c++"), Some(RuntimeId::Native));
        assert_eq!(RuntimeId::from_name("go"), Some(RuntimeId::Native));
        assert_eq!(RuntimeId::from_name(".NET"), Some(RuntimeId::DotNet));
        assert_eq!(RuntimeId::from_name("dotnet"), Some(RuntimeId::DotNet));
        assert_eq!(RuntimeId::from_name("csharp"), Some(RuntimeId::DotNet));
        assert_eq!(RuntimeId::from_name("fsharp"), Some(RuntimeId::DotNet));
        assert_eq!(RuntimeId::from_name("unknown"), None);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", RuntimeId::JVM), "JVM");
        assert_eq!(format!("{}", RuntimeId::Node), "Node");
        assert_eq!(format!("{}", RuntimeId::DotNet), ".NET");
        assert_eq!(format!("{}", RuntimeId::Custom("Deno".to_string())), "Deno");
    }
}
