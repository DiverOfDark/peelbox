crate::define_id_enum! {
    /// Language identifier with support for LLM-discovered languages
    LanguageId {
        Rust => "rust" : "Rust",
        Java => "java" : "Java",
        Kotlin => "kotlin" : "Kotlin",
        JavaScript => "javascript" : "JavaScript",
        TypeScript => "typescript" : "TypeScript",
        Python => "python" : "Python",
        Go => "go" : "Go",
        CSharp => "csharp" : "C#",
        FSharp => "fsharp" : "F#",
        Ruby => "ruby" : "Ruby",
        PHP => "php" : "PHP",
        Cpp => "c++" : "C++",
        Elixir => "elixir" : "Elixir",
        Zig => "zig" : "Zig",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_id_serialization() {
        assert_eq!(
            serde_json::to_string(&LanguageId::Rust).unwrap(),
            "\"rust\""
        );
        assert_eq!(
            serde_json::to_string(&LanguageId::CSharp).unwrap(),
            "\"csharp\""
        );
        assert_eq!(serde_json::to_string(&LanguageId::Cpp).unwrap(), "\"c++\"");
    }

    #[test]
    fn test_language_id_deserialization() {
        assert_eq!(
            serde_json::from_str::<LanguageId>("\"rust\"").unwrap(),
            LanguageId::Rust
        );
        assert_eq!(
            serde_json::from_str::<LanguageId>("\"csharp\"").unwrap(),
            LanguageId::CSharp
        );
    }

    #[test]
    fn test_language_id_name() {
        assert_eq!(LanguageId::Rust.name(), "Rust");
        assert_eq!(LanguageId::CSharp.name(), "C#");
        assert_eq!(LanguageId::FSharp.name(), "F#");
        assert_eq!(LanguageId::Cpp.name(), "C++");
    }

    #[test]
    fn test_custom_language_serialization() {
        let custom = LanguageId::Custom("Zig".to_string());
        assert_eq!(serde_json::to_string(&custom).unwrap(), "\"Zig\"");
    }

    #[test]
    fn test_custom_language_deserialization() {
        let deserialized: LanguageId = serde_json::from_str("\"Zig\"").unwrap();
        assert_eq!(deserialized, LanguageId::Custom("Zig".to_string()));
        assert_eq!(deserialized.name(), "Zig");
    }
}
