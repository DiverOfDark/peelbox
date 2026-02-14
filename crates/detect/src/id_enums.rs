//! ID enum definitions for languages, build systems, frameworks, runtimes, and orchestrators.
//!
//! These were previously in the peelbox-stack crate; inlined here since peelbox-detect
//! is the sole consumer.

macro_rules! define_id_enum {
    (
        $(#[$enum_meta:meta])*
        $enum_name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $serde_name:literal : $display_name:literal
                $( | $alias:literal )*
            ),* $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $enum_name {
            $(
                $(#[$variant_meta])*
                $variant,
            )*
            Custom(String),
        }

        impl serde::Serialize for $enum_name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let s = match self {
                    $(
                        Self::$variant => $serde_name,
                    )*
                    Self::Custom(name) => name,
                };
                serializer.serialize_str(s)
            }
        }

        impl<'de> serde::Deserialize<'de> for $enum_name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Ok(match s.as_str() {
                    $(
                        $serde_name => Self::$variant,
                    )*
                    _ => Self::Custom(s),
                })
            }
        }

        impl $enum_name {
            pub fn name(&self) -> String {
                match self {
                    $(
                        Self::$variant => $display_name.to_string(),
                    )*
                    Self::Custom(name) => name.clone(),
                }
            }

            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $(
                        $display_name $(| $alias)* => Some(Self::$variant),
                    )*
                    _ => None,
                }
            }

            pub fn all_variants() -> &'static [Self] {
                &[
                    $(
                        Self::$variant,
                    )*
                ]
            }
        }
    };
}

macro_rules! define_id_enum_with_display {
    (
        $(#[$enum_meta:meta])*
        $enum_name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $serde_name:literal : $display_name:literal
                $( | $alias:literal )*
            ),* $(,)?
        }
    ) => {
        define_id_enum! {
            $(#[$enum_meta])*
            $enum_name {
                $(
                    $(#[$variant_meta])*
                    $variant => $serde_name : $display_name
                    $( | $alias )*
                ),*
            }
        }

        impl std::fmt::Display for $enum_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.name())
            }
        }
    };
}

// ── Language ID ─────────────────────────────────────────────────────────────

define_id_enum! {
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
        Deno => "deno" : "Deno",
        Zig => "zig" : "Zig",
    }
}

// ── Build System ID ─────────────────────────────────────────────────────────

define_id_enum! {
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
        Deno => "deno" : "Deno" | "deno",
        Zig => "zig" : "Zig Build" | "zig",
        Bazel => "bazel" : "Bazel" | "bazel",
        Turborepo => "turborepo" : "Turborepo" | "turborepo",
    }
}

// ── Framework ID ────────────────────────────────────────────────────────────

define_id_enum! {
    /// Framework identifier with support for LLM-discovered frameworks
    FrameworkId {
        SpringBoot => "spring-boot" : "Spring Boot",
        Quarkus => "quarkus" : "Quarkus",
        Micronaut => "micronaut" : "Micronaut",
        Ktor => "ktor" : "Ktor",
        Express => "express" : "Express",
        NextJs => "nextjs" : "Next.js",
        NestJs => "nestjs" : "NestJS",
        Fastify => "fastify" : "Fastify",
        Django => "django" : "Django",
        Flask => "flask" : "Flask",
        FastApi => "fastapi" : "FastAPI",
        Rails => "rails" : "Rails",
        Sinatra => "sinatra" : "Sinatra",
        ActixWeb => "actix-web" : "Actix Web",
        Axum => "axum" : "Axum",
        Gin => "gin" : "Gin",
        Echo => "echo" : "Echo",
        AspNetCore => "aspnet-core" : "ASP.NET Core",
        Laravel => "laravel" : "Laravel",
        Symfony => "symfony" : "Symfony",
        Phoenix => "phoenix" : "Phoenix",
        Zap => "zap" : "Zap",
        // ── Node/NPM — Server ──
        Hono => "hono" : "Hono",
        Koa => "koa" : "Koa",
        H3 => "h3" : "H3",
        Nitro => "nitro" : "Nitro",
        // ── Node/NPM — SSR / Full-stack ──
        Nuxt => "nuxt" : "Nuxt",
        Remix => "remix" : "Remix",
        ReactRouter => "react-router" : "React Router",
        Astro => "astro" : "Astro",
        SvelteKit => "sveltekit" : "SvelteKit",
        Gatsby => "gatsby" : "Gatsby",
        SolidStart => "solid-start" : "SolidStart",
        RedwoodJs => "redwoodjs" : "RedwoodJS",
        Hydrogen => "hydrogen" : "Hydrogen",
        TanStackStart => "tanstack-start" : "TanStack Start",
        Blitz => "blitz" : "Blitz",
        // ── Node/NPM — Frontend Build / SSG ──
        Vite => "vite" : "Vite",
        CreateReactApp => "create-react-app" : "Create React App",
        Angular => "angular" : "Angular",
        VueCli => "vue-cli" : "Vue CLI",
        PreactCli => "preact-cli" : "Preact CLI",
        Ember => "ember" : "Ember",
        Svelte => "svelte" : "Svelte",
        Docusaurus => "docusaurus" : "Docusaurus",
        Eleventy => "eleventy" : "Eleventy",
        VitePress => "vitepress" : "VitePress",
        VuePress => "vuepress" : "VuePress",
        Stencil => "stencil" : "Stencil",
        Parcel => "parcel" : "Parcel",
        Hexo => "hexo" : "Hexo",
        Gridsome => "gridsome" : "Gridsome",
        Brunch => "brunch" : "Brunch",
        Storybook => "storybook" : "Storybook",
        Sanity => "sanity" : "Sanity",
        Sapper => "sapper" : "Sapper",
        Saber => "saber" : "Saber",
        UmiJs => "umijs" : "UmiJS",
        Dojo => "dojo" : "Dojo",
        Polymer => "polymer" : "Polymer",
        // ── Node/NPM — Ionic / Angular variants ──
        IonicAngular => "ionic-angular" : "Ionic Angular",
        IonicReact => "ionic-react" : "Ionic React",
        Scully => "scully" : "Scully",
    }
}

// ── Runtime ID ──────────────────────────────────────────────────────────────

define_id_enum_with_display! {
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
        Deno => "deno" : "Deno" | "deno",
    }
}

// ── Orchestrator ID ─────────────────────────────────────────────────────────

define_id_enum! {
    /// Orchestrator identifier with support for LLM-discovered orchestrators
    OrchestratorId {
        Turborepo => "turborepo" : "Turborepo",
        Nx => "nx" : "Nx",
        Lerna => "lerna" : "Lerna",
        Rush => "rush" : "Rush",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── LanguageId tests ────────────────────────────────────────────────

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

    // ── BuildSystemId tests ─────────────────────────────────────────────

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
    fn test_bazel_build_system_serialization() {
        let bazel = BuildSystemId::Bazel;
        assert_eq!(serde_json::to_string(&bazel).unwrap(), "\"bazel\"");
    }

    #[test]
    fn test_bazel_build_system_deserialization() {
        let deserialized: BuildSystemId = serde_json::from_str("\"bazel\"").unwrap();
        assert_eq!(deserialized, BuildSystemId::Bazel);
        assert_eq!(deserialized.name(), "Bazel");
    }

    #[test]
    fn test_build_system_from_name_with_aliases() {
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

    // ── FrameworkId tests ───────────────────────────────────────────────

    #[test]
    fn test_framework_id_serialization() {
        assert_eq!(
            serde_json::to_string(&FrameworkId::SpringBoot).unwrap(),
            "\"spring-boot\""
        );
        assert_eq!(
            serde_json::to_string(&FrameworkId::NextJs).unwrap(),
            "\"nextjs\""
        );
    }

    #[test]
    fn test_framework_id_name() {
        assert_eq!(FrameworkId::SpringBoot.name(), "Spring Boot");
        assert_eq!(FrameworkId::NextJs.name(), "Next.js");
    }

    #[test]
    fn test_custom_framework_serialization() {
        let custom = FrameworkId::Custom("Fresh".to_string());
        assert_eq!(serde_json::to_string(&custom).unwrap(), "\"Fresh\"");
    }

    #[test]
    fn test_custom_framework_deserialization() {
        let deserialized: FrameworkId = serde_json::from_str("\"qwik\"").unwrap();
        assert_eq!(deserialized, FrameworkId::Custom("qwik".to_string()));
        assert_eq!(deserialized.name(), "qwik");
    }

    // ── RuntimeId tests ─────────────────────────────────────────────────

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
    fn test_runtime_from_name_with_aliases() {
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
    fn test_runtime_display() {
        assert_eq!(format!("{}", RuntimeId::JVM), "JVM");
        assert_eq!(format!("{}", RuntimeId::Node), "Node");
        assert_eq!(format!("{}", RuntimeId::DotNet), ".NET");
        assert_eq!(format!("{}", RuntimeId::Custom("Deno".to_string())), "Deno");
    }
}
