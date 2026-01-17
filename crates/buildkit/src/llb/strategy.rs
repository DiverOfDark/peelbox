use super::builder::LLBBuilder;
use crate::proto::pb;
use anyhow::Result;
use peelbox_core::output::schema::UniversalBuild;

const WOLFI_BASE_IMAGE: &str = "cgr.dev/chainguard/wolfi-base:latest";

pub trait BuildStrategy {
    fn build_graph(&self, builder: &mut LLBBuilder, spec: &UniversalBuild) -> Result<()>;
}

pub struct PeelboxStrategy;

impl BuildStrategy for PeelboxStrategy {
    fn build_graph(&self, builder: &mut LLBBuilder, spec: &UniversalBuild) -> Result<()> {
        let wolfi_base_idx = builder.create_image_source(WOLFI_BASE_IMAGE);
        let glibc_dynamic_idx =
            builder.create_image_source("cgr.dev/chainguard/glibc-dynamic:latest");
        let busybox_idx = builder.create_image_source("cgr.dev/chainguard/busybox:latest");

        let exclude = builder.load_gitignore_patterns();
        let context_idx = builder.create_local_source(&exclude);

        let with_build_packages_idx = if !spec.build.packages.is_empty() {
            let mut packages_list = spec.build.packages.clone();
            packages_list.sort();
            let packages = packages_list.join(" ");
            let meta = pb::Meta {
                args: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    format!("apk add --no-cache {}", packages),
                ],
                env: vec![],
                cwd: "/".to_string(),
                user: String::new(),
                proxy_env: None,
                extra_hosts: vec![],
                hostname: String::new(),
                ulimit: vec![],
                cgroup_parent: String::new(),
                remove_mount_stubs_recursive: false,
            };

            let mounts = vec![
                builder.layer_mount(0, 0, "/"),
                builder.scratch_mount("/tmp"),
            ];

            Some(builder.create_exec(
                vec![(wolfi_base_idx, 0)],
                mounts,
                meta,
                Some("Install build packages".to_string()),
            ))
        } else {
            None
        };

        let base_idx = with_build_packages_idx.unwrap_or(wolfi_base_idx);

        let build_result_idx = if !spec.build.commands.is_empty() {
            let mut last_idx = base_idx;

            let artifact_paths: Vec<String> =
                spec.runtime.copy.iter().map(|c| c.from.clone()).collect();

            let num_commands = spec.build.commands.len();
            for (i, command) in spec.build.commands.iter().enumerate() {
                let is_last = i == num_commands - 1;

                let mut script = format!("cd /build && {}", command);

                if is_last && !artifact_paths.is_empty() {
                    let artifact_cmds: String = artifact_paths
                        .iter()
                        .enumerate()
                        .map(|(idx, path): (usize, &String)| {
                            let src = if path.starts_with('/') {
                                path.clone()
                            } else {
                                format!("/build/{}", path)
                            };
                            format!(
                                "mkdir -p /peelbox-artifacts/{} && cp -rp {} /peelbox-artifacts/{}/res",
                                idx, src, idx
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" && ");

                    script = format!("{} && {}", script, artifact_cmds);
                }

                let mut env_vars: Vec<String> = spec
                    .build
                    .env
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                env_vars.sort();

                let meta = pb::Meta {
                    args: vec!["sh".to_string(), "-c".to_string(), script],
                    env: env_vars,
                    cwd: "/".to_string(),
                    user: String::new(),
                    proxy_env: None,
                    extra_hosts: vec![],
                    hostname: String::new(),
                    ulimit: vec![],
                    cgroup_parent: String::new(),
                    remove_mount_stubs_recursive: false,
                };

                let mut mounts = vec![
                    builder.layer_mount(0, 0, "/"),
                    builder.layer_mount(1, 1, "/build"),
                    builder.scratch_mount("/tmp"),
                ];

                for cache_path in &spec.build.cache {
                    let absolute: String = if cache_path.starts_with('/') {
                        cache_path.clone()
                    } else {
                        format!("/build/{}", cache_path)
                    };
                    mounts.push(builder.cache_mount(&absolute, cache_path));
                }

                let inputs = if i == 0 {
                    vec![(base_idx, 0), (context_idx, 0)]
                } else {
                    vec![(last_idx, 0), (last_idx, 1)]
                };

                last_idx = builder.create_exec(
                    inputs,
                    mounts,
                    meta,
                    Some(format!("Build command {}", i + 1)),
                );
            }
            last_idx
        } else {
            base_idx
        };

        let runtime_packages_idx = if !spec.runtime.packages.is_empty() {
            let mut packages_list = spec.runtime.packages.clone();
            packages_list.sort();
            let packages = packages_list.join(" ");

            let install_meta = pb::Meta {
                args: vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    format!("mkdir -p /runtime-root/etc/apk /runtime-root/var/lib/apk && cp -r /etc/apk/keys /runtime-root/etc/apk/ && echo \"https://packages.wolfi.dev/os\" > /runtime-root/etc/apk/repositories && apk add --root /runtime-root --no-cache --initdb {} && find /runtime-root -name \"*apk*\" -exec rm -rf {{}} +", packages),
                ],
                env: vec!["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()],
                cwd: "/".to_string(),
                user: String::new(),
                proxy_env: None,
                extra_hosts: vec![],
                hostname: String::new(),
                ulimit: vec![],
                cgroup_parent: String::new(),
                remove_mount_stubs_recursive: false,
            };

            let install_mounts = vec![
                builder.readonly_mount(0, "/"),
                builder.layer_mount(-1, 0, "/runtime-root"),
                builder.scratch_mount("/tmp"),
            ];

            let pkg_install_idx = builder.create_exec(
                vec![(wolfi_base_idx, 0)],
                install_mounts,
                install_meta,
                Some("Install runtime packages into clean root".to_string()),
            );

            Some(pkg_install_idx)
        } else {
            None
        };

        let artifacts_idx = if !spec.runtime.copy.is_empty() {
            let mut copy_cmds = Vec::new();

            for (idx, copy) in spec.runtime.copy.iter().enumerate() {
                let src_path = format!("/build-src/peelbox-artifacts/{}/res", idx);
                copy_cmds.push(format!(
                    "mkdir -p $(dirname /target{}) && cp -rp {} /target{}",
                    copy.to, src_path, copy.to
                ));
            }

            let copy_meta = pb::Meta {
                args: vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    copy_cmds.join(" && "),
                ],
                env: vec![
                    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
                ],
                cwd: "/".to_string(),
                user: String::new(),
                proxy_env: None,
                extra_hosts: vec![],
                hostname: String::new(),
                ulimit: vec![],
                cgroup_parent: String::new(),
                remove_mount_stubs_recursive: false,
            };

            let copy_mounts = vec![
                builder.readonly_mount(0, "/"),
                builder.layer_mount(-1, 0, "/target"),
                builder.readonly_mount(1, "/build-src"),
            ];

            Some(builder.create_exec(
                vec![(busybox_idx, 0), (build_result_idx, 0)],
                copy_mounts,
                copy_meta,
                Some("Prepare clean artifact layer".to_string()),
            ))
        } else {
            None
        };

        let mut merge_inputs = vec![(glibc_dynamic_idx, 0)];
        if let Some(pkg_idx) = runtime_packages_idx {
            merge_inputs.push((pkg_idx, 0));
        }
        if let Some(art_idx) = artifacts_idx {
            merge_inputs.push((art_idx, 0));
        }

        let final_image_idx = builder.create_merge(merge_inputs);
        let _ = builder.create_output_reference(final_image_idx);
        Ok(())
    }
}
