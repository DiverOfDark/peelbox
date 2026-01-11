use crate::buildkit::proto::pb;
use crate::output::schema::UniversalBuild;
use anyhow::Result;
use prost::Message as ProstMessage;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::debug;

const WOLFI_BASE_IMAGE: &str = "cgr.dev/chainguard/wolfi-base:latest";

pub struct LLBBuilder {
    context_name: String,
    context_path: Option<PathBuf>,
    project_name: Option<String>,
    session_id: Option<String>,

    ops: Vec<pb::Op>,
    digests: Vec<String>,
}

impl LLBBuilder {
    pub fn new(context_name: impl Into<String>) -> Self {
        Self {
            context_name: context_name.into(),
            context_path: None,
            project_name: None,
            session_id: None,
            ops: Vec::new(),
            digests: Vec::new(),
        }
    }

    pub fn with_context_path(mut self, context_path: PathBuf) -> Self {
        self.context_path = Some(context_path);
        self
    }

    pub fn with_project_name(mut self, project_name: String) -> Self {
        self.project_name = Some(project_name);
        self
    }

    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    fn add_op(&mut self, mut op: pb::Op) -> i64 {
        let index = self.ops.len() as i64;

        if op.platform.is_none() {
            op.platform = Some(pb::Platform {
                architecture: "amd64".to_string(),
                os: "linux".to_string(),
                variant: String::new(),
                os_version: String::new(),
                os_features: vec![],
            });
        }

        let mut buf = Vec::new();
        op.encode(&mut buf).expect("Failed to encode op");
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(&buf)));

        self.ops.push(op);
        self.digests.push(digest);

        index
    }

    fn get_cache_id(&self, cache_path: &str) -> String {
        let project_name = self.project_name.as_deref().unwrap_or("default");
        let normalized = cache_path.trim_start_matches("/build/").replace('/', "-");
        format!("{}-{}", project_name, normalized)
    }

    fn load_gitignore_patterns(&self) -> Vec<String> {
        let context_root = self
            .context_path
            .as_deref()
            .unwrap_or_else(|| Path::new("."));
        let gitignore_path = context_root.join(".gitignore");

        let mut patterns = Vec::new();

        if gitignore_path.exists() {
            if let Ok(content) = fs::read_to_string(&gitignore_path) {
                for line in content.lines() {
                    let line = line.trim();
                    if !line.is_empty() && !line.starts_with('#') {
                        patterns.push(line.to_string());
                    }
                }
                debug!("Loaded {} patterns from .gitignore", patterns.len());
            }
        }

        patterns.extend(vec![
            ".git/".to_string(),
            ".gitignore".to_string(),
            "*.md".to_string(),
            "LICENSE".to_string(),
            ".vscode/".to_string(),
            ".idea/".to_string(),
            ".buildkit-cache/".to_string(),
            "*.tar".to_string(),
        ]);

        patterns.sort();

        patterns
    }

    fn create_merge(&mut self, inputs: Vec<(i64, i64)>) -> i64 {
        let op_inputs: Vec<pb::Input> = inputs
            .iter()
            .map(|&(input_idx, output_idx)| pb::Input {
                digest: self.digests[input_idx as usize].clone(),
                index: output_idx,
            })
            .collect();

        let merge_inputs = inputs
            .iter()
            .enumerate()
            .map(|(i, _)| pb::MergeInput { input: i as i64 })
            .collect();

        let op = pb::Op {
            inputs: op_inputs,
            op: Some(pb::op::Op::Merge(pb::MergeOp {
                inputs: merge_inputs,
            })),
            platform: None,
            constraints: None,
        };

        self.add_op(op)
    }

    fn create_empty_dir(&mut self) -> i64 {
        let op = pb::Op {
            inputs: vec![],
            op: Some(pb::op::Op::File(pb::FileOp {
                actions: vec![pb::FileAction {
                    input: -1,
                    secondary_input: -1,
                    output: 0,
                    action: Some(pb::file_action::Action::Mkdir(pb::FileActionMkDir {
                        path: "/".to_string(),
                        mode: 0o755,
                        make_parents: true,
                        owner: None,
                        timestamp: -1,
                    })),
                }],
            })),
            platform: None,
            constraints: None,
        };
        self.add_op(op)
    }

    fn create_image_source(&mut self, image_ref: &str) -> i64 {
        let op = pb::Op {
            inputs: vec![],
            op: Some(pb::op::Op::Source(pb::SourceOp {
                identifier: format!("docker-image://{}", image_ref),
                attrs: HashMap::new(),
            })),
            platform: None,
            constraints: None,
        };
        self.add_op(op)
    }

    fn create_output_reference(&mut self, input_idx: i64) -> i64 {
        let op = pb::Op {
            inputs: vec![pb::Input {
                digest: self.digests[input_idx as usize].clone(),
                index: 0,
            }],
            op: None,
            platform: None,
            constraints: None,
        };
        self.add_op(op)
    }

    fn create_local_source(&mut self, exclude_patterns: &[String]) -> i64 {
        let mut attrs = HashMap::new();

        if !exclude_patterns.is_empty() {
            attrs.insert("exclude-patterns".to_string(), exclude_patterns.join(","));
        }

        let shared_key = self
            .project_name
            .as_deref()
            .unwrap_or(&self.context_name)
            .to_string();
        attrs.insert("local.sharedkey".to_string(), shared_key);

        let op = pb::Op {
            inputs: vec![],
            op: Some(pb::op::Op::Source(pb::SourceOp {
                identifier: format!("local://{}", self.context_name),
                attrs,
            })),
            platform: None,
            constraints: None,
        };
        self.add_op(op)
    }

    fn create_exec(
        &mut self,
        inputs: Vec<(i64, i64)>,
        mounts: Vec<pb::Mount>,
        meta: pb::Meta,
        _name: Option<String>,
    ) -> i64 {
        let op_inputs: Vec<pb::Input> = inputs
            .iter()
            .map(|&(input_idx, output_idx)| pb::Input {
                digest: self.digests[input_idx as usize].clone(),
                index: output_idx,
            })
            .collect();

        let op = pb::Op {
            inputs: op_inputs,
            op: Some(pb::op::Op::Exec(pb::ExecOp {
                meta: Some(meta),
                mounts,
                network: pb::NetMode::Unset as i32,
                security: pb::SecurityMode::Sandbox as i32,
                secretenv: vec![],
            })),
            platform: None,
            constraints: None,
        };

        self.add_op(op)
    }

    fn cache_mount(&self, dest: &str, cache_path: &str) -> pb::Mount {
        pb::Mount {
            input: -1,
            selector: String::new(),
            dest: dest.to_string(),
            output: -1,
            readonly: false,
            mount_type: pb::MountType::Cache as i32,
            tmpfs_opt: None,
            cache_opt: Some(pb::CacheOpt {
                id: self.get_cache_id(cache_path),
                sharing: pb::CacheSharingOpt::Shared as i32,
            }),
            secret_opt: None,
            ssh_opt: None,
            result_id: String::new(),
        }
    }

    fn layer_mount(&self, input_idx: i64, output_idx: i64, dest: &str) -> pb::Mount {
        pb::Mount {
            input: input_idx,
            selector: String::new(),
            dest: dest.to_string(),
            output: output_idx,
            readonly: false,
            mount_type: pb::MountType::Bind as i32,
            tmpfs_opt: None,
            cache_opt: None,
            secret_opt: None,
            ssh_opt: None,
            result_id: String::new(),
        }
    }

    fn readonly_mount(&self, input_idx: i64, dest: &str) -> pb::Mount {
        pb::Mount {
            input: input_idx,
            selector: String::new(),
            dest: dest.to_string(),
            output: -1,
            readonly: true,
            mount_type: pb::MountType::Bind as i32,
            tmpfs_opt: None,
            cache_opt: None,
            secret_opt: None,
            ssh_opt: None,
            result_id: String::new(),
        }
    }

    fn scratch_mount(&self, dest: &str) -> pb::Mount {
        pb::Mount {
            input: -1,
            selector: String::new(),
            dest: dest.to_string(),
            output: -1,
            readonly: false,
            mount_type: pb::MountType::Tmpfs as i32,
            tmpfs_opt: Some(pb::TmpfsOpt { size: 0 }),
            cache_opt: None,
            secret_opt: None,
            ssh_opt: None,
            result_id: String::new(),
        }
    }

    pub fn to_bytes(&mut self, spec: &UniversalBuild) -> Result<Vec<u8>> {
        self.build_graph(spec)?;

        let mut def_bytes = Vec::new();
        for op in &self.ops {
            let mut op_bytes = Vec::new();
            op.encode(&mut op_bytes)?;
            def_bytes.push(op_bytes);
        }

        let definition = pb::Definition {
            def: def_bytes,
            metadata: HashMap::new(),
            source: None,
        };

        let mut buf = Vec::new();
        definition.encode(&mut buf)?;

        Ok(buf)
    }

    fn build_graph(&mut self, spec: &UniversalBuild) -> Result<()> {
        let wolfi_base_idx = self.create_image_source(WOLFI_BASE_IMAGE);
        let glibc_dynamic_idx = self.create_image_source("cgr.dev/chainguard/glibc-dynamic:latest");
        let busybox_idx = self.create_image_source("cgr.dev/chainguard/busybox:latest");

        let exclude = self.load_gitignore_patterns();
        let context_idx = self.create_local_source(&exclude);

        let with_build_packages_idx = if !spec.build.packages.is_empty() {
            let packages = spec.build.packages.join(" ");
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

            let mounts = vec![self.layer_mount(0, 0, "/"), self.scratch_mount("/tmp")];

            Some(self.create_exec(
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

                let mut script = if i == 0 {
                    format!(
                        "mkdir -p /build && cp -r /context/. /build && cd /build && {}",
                        command
                    )
                } else {
                    format!("cd /build && {}", command)
                };

                if is_last && !artifact_paths.is_empty() {
                    let artifact_cmds: String = artifact_paths
                        .iter()
                        .enumerate()
                        .map(|(idx, path)| {
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

                let mut mounts = if i == 0 {
                    vec![
                        self.layer_mount(0, 0, "/"),
                        self.readonly_mount(1, "/context"),
                        self.scratch_mount("/tmp"),
                    ]
                } else {
                    vec![self.layer_mount(0, 0, "/"), self.scratch_mount("/tmp")]
                };

                for cache_path in &spec.build.cache {
                    let absolute = if cache_path.starts_with('/') {
                        cache_path.clone()
                    } else {
                        format!("/build/{}", cache_path)
                    };
                    mounts.push(self.cache_mount(&absolute, cache_path));
                }

                let inputs = if i == 0 {
                    vec![(base_idx, 0), (context_idx, 0)]
                } else {
                    vec![(last_idx, 0)]
                };

                last_idx = self.create_exec(
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
            let packages = spec.runtime.packages.join(" ");
            let empty_dir_idx = self.create_empty_dir();

            let install_meta = pb::Meta {
                args: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    format!("apk add --root /runtime-root --no-cache --initdb --repository https://packages.wolfi.dev/os --keys-dir /etc/apk/keys {} && find /runtime-root -name \"*apk*\" -exec rm -rf {{}} +", packages),
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

            let install_mounts = vec![
                self.readonly_mount(0, "/"),
                self.layer_mount(1, 0, "/runtime-root"),
                self.scratch_mount("/tmp"),
            ];

            let pkg_install_idx = self.create_exec(
                vec![(wolfi_base_idx, 0), (empty_dir_idx, 0)],
                install_mounts,
                install_meta,
                Some("Install runtime packages into clean root".to_string()),
            );

            Some(pkg_install_idx)
        } else {
            None
        };

        let mut merge_inputs = vec![(glibc_dynamic_idx, 0)];
        if let Some(pkg_idx) = runtime_packages_idx {
            merge_inputs.push((pkg_idx, 0));
        }

        let squashed_idx = self.create_merge(merge_inputs);

        let final_idx = if spec.runtime.copy.is_empty() {
            squashed_idx
        } else {
            let mut copy_cmds = Vec::new();

            for (idx, copy) in spec.runtime.copy.iter().enumerate() {
                let src_path = format!("/build-src/peelbox-artifacts/{}/res", idx);

                copy_cmds.push(format!(
                    "mkdir -p $(dirname /target{}) && cp -rp {} /target{}",
                    copy.to, src_path, copy.to
                ));
            }

            let copy_script = copy_cmds.join(" && ");

            let copy_meta = pb::Meta {
                args: vec!["sh".to_string(), "-c".to_string(), copy_script],
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

            let copy_mounts = vec![
                self.layer_mount(0, 0, "/"),
                self.layer_mount(1, 0, "/target"),
                self.readonly_mount(2, "/build-src"),
                self.scratch_mount("/tmp"),
            ];

            self.create_exec(
                vec![(busybox_idx, 0), (squashed_idx, 0), (build_result_idx, 0)],
                copy_mounts,
                copy_meta,
                Some("Copy build artifacts".to_string()),
            )
        };

        let truly_final_idx = {
            let cleanup_meta = pb::Meta {
                args: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "find /target -name \"*apk*\" -exec rm -rf {} +".to_string(),
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

            let cleanup_mounts = vec![
                self.layer_mount(0, 0, "/"),
                self.layer_mount(1, 0, "/target"),
            ];

            self.create_exec(
                vec![(busybox_idx, 0), (final_idx, 0)],
                cleanup_mounts,
                cleanup_meta,
                Some("Final distroless cleanup".to_string()),
            )
        };

        let _ = self.create_output_reference(truly_final_idx);
        Ok(())
    }

    pub fn write_definition<W: Write>(
        &mut self,
        spec: &UniversalBuild,
        mut writer: W,
    ) -> Result<()> {
        let bytes = self.to_bytes(spec)?;
        writer.write_all(&bytes)?;
        Ok(())
    }
}
