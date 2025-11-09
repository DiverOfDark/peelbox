use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestPracticeTemplate {
    pub build_stage: BuildStageTemplate,
    pub runtime_stage: RuntimeStageTemplate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildStageTemplate {
    pub base_image: String,
    pub system_packages: Vec<String>,
    pub build_commands: Vec<String>,
    pub cache_paths: Vec<String>,
    pub common_artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStageTemplate {
    pub base_image: String,
    pub system_packages: Vec<String>,
    pub common_ports: Vec<u16>,
}

pub struct BestPractices;

impl BestPractices {
    pub fn get_template(language: &str, build_system: &str) -> Result<BestPracticeTemplate> {
        let key = format!("{}+{}", language.to_lowercase(), build_system.to_lowercase());

        match key.as_str() {
            "rust+cargo" => Ok(Self::rust_cargo()),
            "javascript+npm" | "typescript+npm" => Ok(Self::javascript_npm()),
            "javascript+yarn" | "typescript+yarn" => Ok(Self::javascript_yarn()),
            "javascript+pnpm" | "typescript+pnpm" => Ok(Self::javascript_pnpm()),
            "javascript+bun" | "typescript+bun" => Ok(Self::javascript_bun()),
            "java+maven" => Ok(Self::java_maven()),
            "java+gradle" | "kotlin+gradle" => Ok(Self::java_gradle()),
            "python+pip" => Ok(Self::python_pip()),
            "python+poetry" => Ok(Self::python_poetry()),
            "python+pipenv" => Ok(Self::python_pipenv()),
            "go+go mod" | "go+go" => Ok(Self::go_mod()),
            "c+++cmake" | "c+cmake" | "cpp+cmake" => Ok(Self::cpp_cmake()),
            "c+++make" | "c+make" | "cpp+make" => Ok(Self::cpp_make()),
            ".net+dotnet" | "csharp+dotnet" | "c#+dotnet" => Ok(Self::dotnet()),
            "ruby+bundler" => Ok(Self::ruby_bundler()),
            _ => Err(anyhow!(
                "No best practice template found for language '{}' with build system '{}'",
                language,
                build_system
            )),
        }
    }

    fn rust_cargo() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "rust:1.75".to_string(),
                system_packages: vec!["pkg-config".to_string(), "libssl-dev".to_string()],
                build_commands: vec!["cargo build --release".to_string()],
                cache_paths: vec![
                    "target/".to_string(),
                    "/usr/local/cargo/registry/".to_string(),
                ],
                common_artifacts: vec!["target/release/{project_name}".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "debian:bookworm-slim".to_string(),
                system_packages: vec!["ca-certificates".to_string(), "libssl3".to_string()],
                common_ports: vec![8080],
            },
        }
    }

    fn javascript_npm() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "node:20".to_string(),
                system_packages: vec![],
                build_commands: vec!["npm ci".to_string(), "npm run build".to_string()],
                cache_paths: vec!["node_modules/".to_string(), ".npm/".to_string()],
                common_artifacts: vec!["dist/".to_string(), "build/".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "node:20-slim".to_string(),
                system_packages: vec![],
                common_ports: vec![3000, 8080],
            },
        }
    }

    fn javascript_yarn() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "node:20".to_string(),
                system_packages: vec![],
                build_commands: vec![
                    "yarn install --frozen-lockfile".to_string(),
                    "yarn build".to_string(),
                ],
                cache_paths: vec![
                    "node_modules/".to_string(),
                    ".yarn/cache/".to_string(),
                ],
                common_artifacts: vec!["dist/".to_string(), "build/".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "node:20-slim".to_string(),
                system_packages: vec![],
                common_ports: vec![3000, 8080],
            },
        }
    }

    fn javascript_pnpm() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "node:20".to_string(),
                system_packages: vec![],
                build_commands: vec![
                    "corepack enable".to_string(),
                    "pnpm install --frozen-lockfile".to_string(),
                    "pnpm build".to_string(),
                ],
                cache_paths: vec![
                    "node_modules/".to_string(),
                    ".pnpm-store/".to_string(),
                ],
                common_artifacts: vec!["dist/".to_string(), "build/".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "node:20-slim".to_string(),
                system_packages: vec![],
                common_ports: vec![3000, 8080],
            },
        }
    }

    fn javascript_bun() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "oven/bun:1".to_string(),
                system_packages: vec![],
                build_commands: vec!["bun install".to_string(), "bun run build".to_string()],
                cache_paths: vec!["node_modules/".to_string(), ".bun/".to_string()],
                common_artifacts: vec!["dist/".to_string(), "build/".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "oven/bun:1-slim".to_string(),
                system_packages: vec![],
                common_ports: vec![3000, 8080],
            },
        }
    }

    fn java_maven() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "maven:3.9-eclipse-temurin-21".to_string(),
                system_packages: vec![],
                build_commands: vec!["mvn clean package -DskipTests".to_string()],
                cache_paths: vec!["/root/.m2/repository/".to_string()],
                common_artifacts: vec!["target/*.jar".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "eclipse-temurin:21-jre".to_string(),
                system_packages: vec![],
                common_ports: vec![8080],
            },
        }
    }

    fn java_gradle() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "gradle:8.5-jdk21".to_string(),
                system_packages: vec![],
                build_commands: vec!["gradle build -x test".to_string()],
                cache_paths: vec![
                    "/root/.gradle/caches/".to_string(),
                    "/root/.gradle/wrapper/".to_string(),
                ],
                common_artifacts: vec!["build/libs/*.jar".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "eclipse-temurin:21-jre".to_string(),
                system_packages: vec![],
                common_ports: vec![8080],
            },
        }
    }

    fn python_pip() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "python:3.11".to_string(),
                system_packages: vec!["build-essential".to_string()],
                build_commands: vec!["pip install --no-cache-dir -r requirements.txt".to_string()],
                cache_paths: vec!["/root/.cache/pip/".to_string()],
                common_artifacts: vec![
                    "/usr/local/lib/python3.11/site-packages".to_string(),
                    "app/".to_string(),
                ],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "python:3.11-slim".to_string(),
                system_packages: vec![],
                common_ports: vec![8000, 5000],
            },
        }
    }

    fn python_poetry() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "python:3.11".to_string(),
                system_packages: vec!["build-essential".to_string()],
                build_commands: vec![
                    "pip install poetry".to_string(),
                    "poetry install --no-dev".to_string(),
                ],
                cache_paths: vec![
                    ".venv/".to_string(),
                    "/root/.cache/pypoetry/".to_string(),
                ],
                common_artifacts: vec!["dist/".to_string(), ".venv/".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "python:3.11-slim".to_string(),
                system_packages: vec![],
                common_ports: vec![8000, 5000],
            },
        }
    }

    fn python_pipenv() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "python:3.11".to_string(),
                system_packages: vec!["build-essential".to_string()],
                build_commands: vec![
                    "pip install pipenv".to_string(),
                    "pipenv install --deploy".to_string(),
                ],
                cache_paths: vec![
                    "/root/.cache/pip/".to_string(),
                    "/root/.cache/pipenv/".to_string(),
                ],
                common_artifacts: vec!["Pipfile".to_string(), "Pipfile.lock".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "python:3.11-slim".to_string(),
                system_packages: vec![],
                common_ports: vec![8000, 5000],
            },
        }
    }

    fn go_mod() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "golang:1.21".to_string(),
                system_packages: vec![],
                build_commands: vec!["go build -o app .".to_string()],
                cache_paths: vec![
                    "/go/pkg/mod/".to_string(),
                    "/root/.cache/go-build/".to_string(),
                ],
                common_artifacts: vec!["app".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "alpine:3.19".to_string(),
                system_packages: vec!["ca-certificates".to_string()],
                common_ports: vec![8080],
            },
        }
    }

    fn cpp_cmake() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "gcc:13".to_string(),
                system_packages: vec!["cmake".to_string(), "make".to_string()],
                build_commands: vec![
                    "cmake -B build -DCMAKE_BUILD_TYPE=Release".to_string(),
                    "cmake --build build --config Release".to_string(),
                ],
                cache_paths: vec!["build/".to_string()],
                common_artifacts: vec!["build/{project_name}".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "debian:bookworm-slim".to_string(),
                system_packages: vec!["libstdc++6".to_string()],
                common_ports: vec![8080],
            },
        }
    }

    fn cpp_make() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "gcc:13".to_string(),
                system_packages: vec!["make".to_string()],
                build_commands: vec!["make".to_string()],
                cache_paths: vec![],
                common_artifacts: vec!["{project_name}".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "debian:bookworm-slim".to_string(),
                system_packages: vec!["libstdc++6".to_string()],
                common_ports: vec![8080],
            },
        }
    }

    fn dotnet() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "mcr.microsoft.com/dotnet/sdk:8.0".to_string(),
                system_packages: vec![],
                build_commands: vec![
                    "dotnet restore".to_string(),
                    "dotnet publish -c Release -o out".to_string(),
                ],
                cache_paths: vec![
                    "/root/.nuget/packages/".to_string(),
                    "obj/".to_string(),
                ],
                common_artifacts: vec!["out/".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "mcr.microsoft.com/dotnet/aspnet:8.0".to_string(),
                system_packages: vec![],
                common_ports: vec![8080, 5000],
            },
        }
    }

    fn ruby_bundler() -> BestPracticeTemplate {
        BestPracticeTemplate {
            build_stage: BuildStageTemplate {
                base_image: "ruby:3.2".to_string(),
                system_packages: vec!["build-essential".to_string()],
                build_commands: vec![
                    "bundle config set --local deployment 'true'".to_string(),
                    "bundle install".to_string(),
                ],
                cache_paths: vec!["vendor/bundle/".to_string()],
                common_artifacts: vec!["vendor/bundle/".to_string(), "app/".to_string()],
            },
            runtime_stage: RuntimeStageTemplate {
                base_image: "ruby:3.2-slim".to_string(),
                system_packages: vec![],
                common_ports: vec![3000],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_cargo_template() {
        let template = BestPractices::get_template("rust", "cargo").unwrap();
        assert_eq!(template.build_stage.base_image, "rust:1.75");
        assert_eq!(template.runtime_stage.base_image, "debian:bookworm-slim");
        assert!(!template.build_stage.build_commands.is_empty());
    }

    #[test]
    fn test_javascript_npm_template() {
        let template = BestPractices::get_template("javascript", "npm").unwrap();
        assert_eq!(template.build_stage.base_image, "node:20");
        assert_eq!(template.runtime_stage.base_image, "node:20-slim");
    }

    #[test]
    fn test_typescript_npm_template() {
        let template = BestPractices::get_template("typescript", "npm").unwrap();
        assert_eq!(template.build_stage.base_image, "node:20");
    }

    #[test]
    fn test_java_maven_template() {
        let template = BestPractices::get_template("java", "maven").unwrap();
        assert!(template.build_stage.base_image.contains("maven"));
        assert!(template.runtime_stage.base_image.contains("temurin"));
    }

    #[test]
    fn test_python_poetry_template() {
        let template = BestPractices::get_template("python", "poetry").unwrap();
        assert_eq!(template.build_stage.base_image, "python:3.11");
        assert!(template.build_stage.build_commands.iter().any(|c| c.contains("poetry")));
    }

    #[test]
    fn test_go_mod_template() {
        let template = BestPractices::get_template("go", "go mod").unwrap();
        assert!(template.build_stage.base_image.contains("golang"));
        assert_eq!(template.runtime_stage.base_image, "alpine:3.19");
    }

    #[test]
    fn test_dotnet_template() {
        let template = BestPractices::get_template(".net", "dotnet").unwrap();
        assert!(template.build_stage.base_image.contains("dotnet/sdk"));
        assert!(template.runtime_stage.base_image.contains("dotnet/aspnet"));
    }

    #[test]
    fn test_unsupported_combination() {
        let result = BestPractices::get_template("cobol", "make");
        assert!(result.is_err());
    }

    #[test]
    fn test_case_insensitive() {
        let template1 = BestPractices::get_template("Rust", "Cargo").unwrap();
        let template2 = BestPractices::get_template("RUST", "CARGO").unwrap();
        assert_eq!(template1.build_stage.base_image, template2.build_stage.base_image);
    }

    #[test]
    fn test_all_templates_have_required_fields() {
        let combinations = vec![
            ("rust", "cargo"),
            ("javascript", "npm"),
            ("javascript", "yarn"),
            ("javascript", "pnpm"),
            ("javascript", "bun"),
            ("java", "maven"),
            ("java", "gradle"),
            ("python", "pip"),
            ("python", "poetry"),
            ("python", "pipenv"),
            ("go", "go mod"),
            ("c++", "cmake"),
            ("c++", "make"),
            (".net", "dotnet"),
            ("ruby", "bundler"),
        ];

        for (lang, build_sys) in combinations {
            let template = BestPractices::get_template(lang, build_sys)
                .unwrap_or_else(|_| panic!("Failed to get template for {} + {}", lang, build_sys));

            assert!(!template.build_stage.base_image.is_empty(), "{} + {}: build base image empty", lang, build_sys);
            assert!(!template.runtime_stage.base_image.is_empty(), "{} + {}: runtime base image empty", lang, build_sys);
            assert!(!template.build_stage.build_commands.is_empty(), "{} + {}: build commands empty", lang, build_sys);
        }
    }

    #[test]
    fn test_template_serialization() {
        let template = BestPractices::get_template("rust", "cargo").unwrap();
        let json = serde_json::to_string(&template).unwrap();
        assert!(json.contains("rust:1.75"));
        assert!(json.contains("debian:bookworm-slim"));

        let deserialized: BestPracticeTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.build_stage.base_image, template.build_stage.base_image);
    }
}
