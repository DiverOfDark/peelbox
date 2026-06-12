use super::*;

pub(crate) fn reduce(bucket: ServiceBucket, registry: &Registry) -> Result<UniversalBuild> {
    let m = &bucket.manifest;
    let profile = registry.get_profile(&m.build_system);

    // Check if this is a standalone project in a subdirectory
    let is_subdirectory = !bucket.is_workspace_member
        && !bucket.path.as_os_str().is_empty()
        && bucket.path != Path::new(".");
    let subdir = bucket.path.to_string_lossy().to_string();

    // Resolve build commands (workspace-aware or subdirectory-aware)
    let build_commands = if bucket.is_workspace_member {
        if let Some(transform) = &m.build.member_transform {
            transform
                .member_commands
                .iter()
                .map(|cmd| {
                    cmd.replace("{module}", &bucket.module_name())
                        .replace("{package}", &bucket.package_name())
                        .replace("{root}", &bucket.workspace_root_display())
                })
                .collect()
        } else {
            m.build.commands.clone()
        }
    } else if is_subdirectory {
        // For standalone projects in subdirectories, delegate to build system profile
        let transform = profile
            .map(|p| p.transform_subdirectory_command)
            .unwrap_or(default_subdirectory_command);
        m.build
            .commands
            .iter()
            .map(|cmd| transform(cmd, &subdir))
            .collect()
    } else {
        m.build.commands.clone()
    };

    // Resolve artifacts (workspace-aware or subdirectory-aware)
    let mut artifacts: Vec<CopySpec> = if bucket.is_workspace_member {
        if let Some(transform) = &m.build.member_transform {
            if let Some(member_artifacts) = &transform.member_artifacts {
                member_artifacts
                    .iter()
                    .map(|(from, to)| CopySpec {
                        from: from
                            .replace("{module}", &bucket.module_name())
                            .replace("{package}", &bucket.package_name()),
                        to: to
                            .replace("{module}", &bucket.module_name())
                            .replace("{package}", &bucket.package_name()),
                    })
                    .collect()
            } else {
                m.build
                    .artifacts
                    .iter()
                    .map(|(from, to)| CopySpec {
                        from: from
                            .replace("{module}", &bucket.module_name())
                            .replace("{package}", &bucket.package_name()),
                        to: to
                            .replace("{module}", &bucket.module_name())
                            .replace("{package}", &bucket.package_name()),
                    })
                    .collect()
            }
        } else {
            m.build
                .artifacts
                .iter()
                .map(|(from, to)| CopySpec {
                    from: from.clone(),
                    to: to.clone(),
                })
                .collect()
        }
    } else if is_subdirectory {
        // For standalone subdirectory projects, prepend directory to artifact paths
        // Exception: build systems with shared_target_dir (e.g., Cargo uses --target-dir target)
        let uses_shared_target = profile.is_some_and(|p| p.shared_target_dir);
        m.build
            .artifacts
            .iter()
            .map(|(from, to)| CopySpec {
                from: if from.starts_with('/') || from.starts_with('.') || uses_shared_target {
                    from.clone()
                } else {
                    format!("{}/{}", subdir, from)
                },
                to: to.clone(),
            })
            .collect()
    } else {
        m.build
            .artifacts
            .iter()
            .map(|(from, to)| CopySpec {
                from: from.clone(),
                to: to.clone(),
            })
            .collect()
    };

    // Delegate artifact post-processing to build system profile (e.g., Gradle JAR name resolution)
    if let Some(p) = profile {
        (p.resolve_artifacts)(&mut artifacts, m.package.as_ref());
    }

    // Merge config contributions into runtime spec
    let mut runtime_env = m.runtime_config.env.clone();
    let mut runtime_ports = m.runtime_config.ports.clone();
    let mut runtime_packages = m.runtime_config.packages.clone();
    let mut health_endpoint = m.runtime_config.health_endpoint.clone();
    let mut config_runtime_command: Option<String> = None;

    for config in &bucket.configs {
        runtime_env.extend(config.env_vars.clone());
        runtime_ports.extend(config.ports.clone());
        if health_endpoint.is_none() {
            health_endpoint.clone_from(&config.health_endpoint);
        }
        if config_runtime_command.is_none() {
            config_runtime_command.clone_from(&config.runtime_command);
        }
    }

    // Apply framework contribution
    let mut framework_runtime_command = None;
    let mut framework_workdir = None;
    let framework_name = if let Some(fw) = &bucket.framework {
        runtime_env.extend(fw.env_vars.clone());
        runtime_env.extend(fw.runtime_env.clone());
        runtime_packages.extend(fw.runtime_packages.clone());
        // Framework ports always override when specified
        if !fw.default_ports.is_empty() {
            runtime_ports = fw.default_ports.clone();
        }
        if health_endpoint.is_none() {
            health_endpoint = fw.health_endpoints.first().cloned();
        }
        framework_runtime_command = fw.runtime_command.clone();
        framework_workdir = fw.workdir.clone();

        // Framework extra_copy replaces generic artifacts when non-empty
        if !fw.extra_copy.is_empty() {
            artifacts = fw
                .extra_copy
                .iter()
                .map(|(from, to)| CopySpec {
                    from: from.clone(),
                    to: to.clone(),
                })
                .collect();
        }
        Some(fw.framework.name())
    } else {
        None
    };

    // When a config provides a runtime command (e.g., Procfile), its ports should
    // take priority over framework defaults since it represents an explicit user declaration.
    // Insert config ports at the front so they are used for health checks (which use the first port).
    if config_runtime_command.is_some() {
        let mut config_ports = Vec::new();
        for config in &bucket.configs {
            if config.runtime_command.is_some() {
                for &port in &config.ports {
                    if !config_ports.contains(&port) {
                        config_ports.push(port);
                    }
                }
            }
        }
        // Remove any config ports already in runtime_ports, then prepend them
        runtime_ports.retain(|p| !config_ports.contains(p));
        config_ports.extend(runtime_ports);
        runtime_ports = config_ports;
    }

    // When framework sets workdir, update artifact copy targets from /app to framework workdir
    if let Some(ref fw_workdir) = framework_workdir {
        if fw_workdir != "/app" {
            for artifact in &mut artifacts {
                if artifact.to == "/app" || artifact.to == "/app/" {
                    artifact.to = fw_workdir.clone();
                }
            }
        }
    }

    // Deduplicate (preserve insertion order, remove later duplicates)
    {
        let mut seen = std::collections::HashSet::new();
        runtime_ports.retain(|p| seen.insert(*p));
    }
    {
        let mut seen = std::collections::HashSet::new();
        runtime_packages.retain(|p| seen.insert(p.clone()));
    }

    // Determine project name
    let is_root_project = bucket.path.as_os_str().is_empty() || bucket.path == Path::new(".");
    let project_name = if is_root_project {
        // For root-level projects: use package name only from strong naming sources,
        // unless the build system profile opts out (e.g., Gradle, Poetry, Pip)
        if profile.is_some_and(|p| !p.use_package_name_for_root) {
            Some("app".into())
        } else {
            m.package
                .as_ref()
                .filter(|p| !p.name.is_empty())
                .map(|p| p.name.clone())
                .or(Some("app".into()))
        }
    } else {
        // Non-root: package name or directory name
        m.package
            .as_ref()
            .filter(|p| !p.name.is_empty())
            .map(|p| p.name.clone())
            .or_else(|| {
                bucket
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            })
    };

    // Build entrypoint command: config (Procfile) > framework override > manifest entrypoint
    let mut entrypoint_cmd = if let Some(config_cmd) = config_runtime_command {
        // Extract leading KEY=VALUE tokens as environment variables so they don't
        // end up as the executable in the command array.
        let parts: Vec<&str> = config_cmd.split_whitespace().collect();
        let env_prefix_end = parts
            .iter()
            .position(|p| !p.contains('=') || p.starts_with('/') || p.starts_with('.'))
            .unwrap_or(parts.len());
        for kv in &parts[..env_prefix_end] {
            if let Some((k, v)) = kv.split_once('=') {
                runtime_env
                    .entry(k.to_string())
                    .or_insert_with(|| v.to_string());
            }
        }
        let remaining: &str = &config_cmd[config_cmd
            .find(parts.get(env_prefix_end).copied().unwrap_or(""))
            .unwrap_or(0)..];
        let remaining = remaining.trim();
        // If the command contains shell operators, wrap with sh -c to preserve semantics.
        let has_shell_ops = remaining.contains("&&")
            || remaining.contains("||")
            || remaining.contains('|')
            || remaining.contains(';')
            || remaining.contains("$(")
            || remaining.contains("${");
        if has_shell_ops {
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                remaining.to_string(),
            ]
        } else {
            remaining.split_whitespace().map(String::from).collect()
        }
    } else if let Some(fw_cmd) = framework_runtime_command {
        fw_cmd
    } else if let Some(entrypoint) = &m.runtime_config.entrypoint {
        // For non-root projects, build system profile may override the entrypoint
        // (e.g., Node.js uses `npm start` instead of direct command).
        // Only apply the override when the package has a start script (is_application),
        // not when the entrypoint was derived from the `main` field.
        let has_start_script = m
            .package
            .as_ref()
            .map(|p| p.is_application)
            .unwrap_or(false);
        if !is_root_project && has_start_script {
            if let Some(override_cmd) = profile.and_then(|p| p.non_root_entrypoint_override) {
                override_cmd.iter().map(|s| s.to_string()).collect()
            } else {
                entrypoint.split_whitespace().map(String::from).collect()
            }
        } else {
            entrypoint.split_whitespace().map(String::from).collect()
        }
    } else {
        vec![]
    };

    // For Gradle installDist projects, resolve the generic `/app/bin/app` entrypoint
    // to `/app/bin/{project_name}` using the actual project name from settings.gradle.
    // installDist creates scripts named after the project, not "app".
    if m.build_system.slug() == "gradle" {
        if let Some(pkg) = &m.package {
            if !pkg.name.is_empty() && pkg.name != "app" {
                for part in entrypoint_cmd.iter_mut() {
                    if *part == "/app/bin/app" {
                        *part = format!("/app/bin/{}", pkg.name);
                    }
                }
            }
        }
    }

    // For Ruby/Bundler projects, ensure `bundle exec` wraps the entrypoint
    // so gems installed in vendor/bundle are on the load path.
    if m.build_system.slug() == "bundler"
        && !entrypoint_cmd.is_empty()
        && entrypoint_cmd.first().map(|s| s.as_str()) != Some("bundle")
        && entrypoint_cmd
            .iter()
            .any(|s| s == "ruby" || s.ends_with(".rb"))
    {
        let mut wrapped = vec!["bundle".to_string(), "exec".to_string()];
        wrapped.extend(entrypoint_cmd);
        entrypoint_cmd = wrapped;
    }

    // For polyglot projects where the entrypoint uses a language not in the primary
    // runtime packages: add the required runtime binaries (e.g., Node+Ruby project
    // where Procfile runs `ruby app.rb` but primary language is JavaScript).
    let cmd_uses_ruby = entrypoint_cmd
        .iter()
        .any(|s| s == "ruby" || s == "bundle" || s.ends_with(".rb"));
    if cmd_uses_ruby && !runtime_packages.iter().any(|p| p.starts_with("ruby")) {
        // Find ruby packages from build packages
        for pkg in &m.build.packages {
            if ((pkg.starts_with("ruby") && !pkg.contains("-dev")) || pkg == "bundler")
                && !runtime_packages.contains(pkg)
            {
                runtime_packages.push(pkg.clone());
            }
        }
    }

    // Workdir: framework override > manifest workdir
    // For workspace members with adjusts_workspace_member_workdir, or standalone
    // subdirectory projects, set workdir to the member/subdir's directory so that
    // the entrypoint command runs in the correct context.
    let needs_subdir_workdir = (bucket.is_workspace_member || is_subdirectory)
        && profile.is_some_and(|p| p.adjusts_workspace_member_workdir);
    let workdir = if needs_subdir_workdir {
        let base = framework_workdir
            .or_else(|| m.runtime_config.workdir.clone())
            .unwrap_or_else(|| "/app".into());
        let member_path = bucket.path.display().to_string();
        if member_path.is_empty() || member_path == "." {
            base
        } else {
            format!("{}/{}", base, member_path)
        }
    } else {
        framework_workdir
            .or_else(|| m.runtime_config.workdir.clone())
            .unwrap_or_else(|| "/app".into())
    };

    Ok(UniversalBuild {
        version: "1.0".into(),
        metadata: BuildMetadata {
            project_name,
            language: m.language.name(),
            build_system: m.build_system.name(),
            framework: framework_name,
            reasoning: {
                let filename = m
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let parent = m.path.parent().filter(|p| !p.as_os_str().is_empty());
                match parent {
                    Some(dir) => format!("Detected from {} in {}", filename, dir.display()),
                    None => format!("Detected from {}", filename),
                }
            },
        },
        build: BuildStage {
            packages: m.build.packages.clone(),
            env: m.build.env.clone(),
            commands: build_commands,
            cache: m.build.cache_dirs.clone(),
        },
        runtime: RuntimeStage {
            packages: runtime_packages,
            env: runtime_env,
            copy: artifacts,
            command: entrypoint_cmd,
            workdir,
            ports: runtime_ports,
            health: health_endpoint.map(|e| HealthCheck { endpoint: e }),
        },
    })
}
