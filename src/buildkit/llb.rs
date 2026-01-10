// Minimal LLB (Low-Level Build) implementation using BuildKit protobufs
// We implement this ourselves for full control over cache mounts

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

    // DAG state
    ops: Vec<pb::Op>,
    digests: Vec<String>,
}

impl LLBBuilder {
    pub fn new(context_name: impl Into<String>) -> Self {
        Self {
            context_name: context_name.into(),
            context_path: None,
            project_name: None,
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

    /// Add an operation to the DAG and return its index
    fn add_op(&mut self, op: pb::Op) -> i64 {
        let index = self.ops.len() as i64;

        // Marshal and compute digest
        let mut buf = Vec::new();
        op.encode(&mut buf).expect("Failed to encode op");
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(&buf)));

        self.ops.push(op);
        self.digests.push(digest);

        index
    }

    /// Generate unique cache ID for parallel build isolation
    fn get_cache_id(&self, cache_path: &str) -> String {
        let project_name = self.project_name.as_deref().unwrap_or("default");
        let normalized = cache_path.trim_start_matches("/build/").replace('/', "-");
        format!("{}-{}", project_name, normalized)
    }

    /// Read .gitignore patterns from context root
    fn load_gitignore_patterns(&self) -> Vec<String> {
        let context_root = self.context_path.as_ref()
            .map(|p| p.as_path())
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

        // Standard exclusions
        patterns.extend(vec![
            ".git".to_string(),
            ".gitignore".to_string(),
            "*.md".to_string(),
            "LICENSE".to_string(),
            ".vscode".to_string(),
            ".idea".to_string(),
        ]);

        patterns
    }

    /// Create image source operation
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

    /// Create output reference node (required for BuildKit provenance)
    /// This is a reference-only operation with no op field, pointing to the final build output
    fn create_output_reference(&mut self, input_idx: i64) -> i64 {
        let op = pb::Op {
            inputs: vec![pb::Input {
                digest: self.digests[input_idx as usize].clone(),
                index: 0,
            }],
            op: None,  // Reference-only node (required for provenance)
            platform: None,
            constraints: None,
        };
        self.add_op(op)
    }

    /// Create local context source operation
    fn create_local_source(&mut self, exclude_patterns: &[String]) -> i64 {
        let mut attrs = HashMap::new();

        // Add exclude patterns
        for (i, pattern) in exclude_patterns.iter().enumerate() {
            attrs.insert(format!("excludepattern.{}", i), pattern.clone());
        }

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

    /// Create exec operation
    fn create_exec(
        &mut self,
        inputs: Vec<i64>,
        mounts: Vec<pb::Mount>,
        meta: pb::Meta,
        _name: Option<String>,
    ) -> i64 {
        let op_inputs: Vec<pb::Input> = inputs
            .iter()
            .map(|&input_idx| pb::Input {
                digest: self.digests[input_idx as usize].clone(),
                index: 0,
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
                cdi_devices: vec![],
            })),
            platform: None,
            constraints: None,
        };

        self.add_op(op)
    }

    /// Create file copy operation (using FileOp for distroless images without shell)
    fn create_file_copy(
        &mut self,
        base_idx: i64,
        src_idx: i64,
        src_path: &str,
        dest_path: &str,
        description: Option<String>,
    ) -> i64 {
        let op_inputs: Vec<pb::Input> = vec![
            pb::Input {
                digest: self.digests[base_idx as usize].clone(),
                index: 0,  // Reference output 0 from base operation
            },
            pb::Input {
                digest: self.digests[src_idx as usize].clone(),
                index: 0,  // Reference output 0 from source operation
            },
        ];

        let action = pb::FileAction {
            input: 1,  // Source is input index 1
            secondary_input: -1,
            output: 0,  // Result goes to output index 0
            action: Some(pb::file_action::Action::Copy(pb::FileActionCopy {
                src: src_path.to_string(),
                dest: dest_path.to_string(),
                owner: None,
                mode: -1,
                mode_str: String::new(),
                follow_symlink: false,
                dir_copy_contents: true,
                attempt_unpack_docker_compatibility: false,
                create_dest_path: true,
                allow_wildcard: true,
                allow_empty_wildcard: true,
                timestamp: -1,
                include_patterns: vec![],
                exclude_patterns: vec![],
                required_paths: vec![],
                always_replace_existing_dest_paths: false,
            })),
        };

        let op = pb::Op {
            inputs: op_inputs,
            op: Some(pb::op::Op::File(pb::FileOp {
                actions: vec![action],
            })),
            platform: None,
            constraints: None,
        };

        let idx = self.add_op(op);

        if let Some(desc) = description {
            debug!("Created file copy: {} (op {})", desc, idx);
        }

        idx
    }

    /// Create cache mount with project-specific ID
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
            content_cache: pb::MountContentCache::Default as i32,
        }
    }

    /// Create layer mount (read-write)
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
            content_cache: pb::MountContentCache::Default as i32,
        }
    }

    /// Create readonly mount
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
            content_cache: pb::MountContentCache::Default as i32,
        }
    }

    /// Create scratch mount (tmpfs) - never persisted as output
    fn scratch_mount(&self, dest: &str) -> pb::Mount {
        pb::Mount {
            input: -1,
            selector: String::new(),
            dest: dest.to_string(),
            output: -1,  // Tmpfs should not be persisted
            readonly: false,
            mount_type: pb::MountType::Tmpfs as i32,
            tmpfs_opt: Some(pb::TmpfsOpt { size: 0 }),
            cache_opt: None,
            secret_opt: None,
            ssh_opt: None,
            result_id: String::new(),
            content_cache: pb::MountContentCache::Default as i32,
        }
    }

    /// Serialize to LLB Definition bytes
    pub fn to_bytes(&mut self, spec: &UniversalBuild) -> Result<Vec<u8>> {
        // Build the LLB graph
        self.build_graph(spec)?;

        // Create Definition
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

    /// Build the LLB operation graph - 4-stage distroless build
    fn build_graph(&mut self, spec: &UniversalBuild) -> Result<()> {
        // Create sources
        let wolfi_base_idx = self.create_image_source(WOLFI_BASE_IMAGE);
        let glibc_dynamic_idx = self.create_image_source("cgr.dev/chainguard/glibc-dynamic:latest");
        let busybox_idx = self.create_image_source("cgr.dev/chainguard/busybox:latest");

        let exclude = self.load_gitignore_patterns();
        let context_idx = self.create_local_source(&exclude);

        // Stage 1: Install build packages
        let with_build_packages_idx = if !spec.build.packages.is_empty() {
            let packages = spec.build.packages.join(" ");
            let meta = pb::Meta {
                args: vec!["sh".to_string(), "-c".to_string(), format!("apk add --no-cache {}", packages)],
                env: vec![],
                cwd: "/".to_string(),
                user: String::new(),
                proxy_env: None,
                extra_hosts: vec![],
                hostname: String::new(),
                ulimit: vec![],
                cgroup_parent: String::new(),
                remove_mount_stubs_recursive: false,
                valid_exit_codes: vec![],
            };

            let mounts = vec![
                self.layer_mount(0, 0, "/"),  // Input 0: wolfi_base_idx
                self.scratch_mount("/tmp"),
            ];

            Some(self.create_exec(vec![wolfi_base_idx], mounts, meta, Some("Install build packages".to_string())))
        } else {
            None
        };

        // Stage 2: Run build commands (simplified single-output approach)
        let base_idx = with_build_packages_idx.unwrap_or(wolfi_base_idx);

        let build_result_idx = if !spec.build.commands.is_empty() {
            debug!("Build stage starting from base_idx={}", base_idx);

            let mut last_idx = base_idx;

            // Use runtime.copy to determine artifact paths
            let artifact_paths: Vec<String> = spec.runtime.copy.iter()
                .map(|c| c.from.clone())
                .collect();

            for (i, command) in spec.build.commands.iter().enumerate() {
                // Build script: copy context to /build, run build, copy artifacts to /artifacts
                let script = if i == 0 {
                    // First command: setup, build, and stage artifacts at root level
                    let artifact_cmds: String = if !artifact_paths.is_empty() {
                        artifact_paths.iter()
                            .map(|path| format!("cp -r {} /artifacts/ 2>/dev/null || true", path))
                            .collect::<Vec<_>>()
                            .join(" && ")
                    } else {
                        String::new()
                    };

                    if artifact_cmds.is_empty() {
                        format!(
                            "mkdir -p /build && cp -r /context/. /build && cd /build && {}",
                            command
                        )
                    } else {
                        format!(
                            "mkdir -p /build && cp -r /context/. /build && cd /build && {} && mkdir -p /artifacts && {}",
                            command, artifact_cmds
                        )
                    }
                } else {
                    // Subsequent commands: continue building and update artifacts
                    let artifact_cmds: String = if !artifact_paths.is_empty() {
                        artifact_paths.iter()
                            .map(|path| format!("cp -r {} /artifacts/ 2>/dev/null || true", path))
                            .collect::<Vec<_>>()
                            .join(" && ")
                    } else {
                        String::new()
                    };

                    if artifact_cmds.is_empty() {
                        format!("cd /build && {}", command)
                    } else {
                        format!("cd /build && {} && {}", command, artifact_cmds)
                    }
                };

                let meta = pb::Meta {
                    args: vec!["sh".to_string(), "-c".to_string(), script],
                    env: spec.build.env.iter().map(|(k, v)| format!("{}={}", k, v)).collect(),
                    cwd: "/".to_string(),
                    user: String::new(),
                    proxy_env: None,
                    extra_hosts: vec![],
                    hostname: String::new(),
                    ulimit: vec![],
                    cgroup_parent: String::new(),
                    remove_mount_stubs_recursive: false,
                    valid_exit_codes: vec![],
                };

                // Simple mount configuration: root (output 0), context (readonly, first command only), tmp
                let mut mounts = if i == 0 {
                    vec![
                        self.layer_mount(0, 0, "/"),        // Input 0: base_idx, Output 0
                        self.readonly_mount(1, "/context"),  // Input 1: context_idx
                        self.scratch_mount("/tmp"),
                    ]
                } else {
                    vec![
                        self.layer_mount(0, 0, "/"),  // Input 0: last_idx, Output 0
                        self.scratch_mount("/tmp"),
                    ]
                };

                // Add cache mounts (working directory /build)
                for cache_path in &spec.build.cache {
                    let absolute = if cache_path.starts_with('/') {
                        cache_path.clone()
                    } else {
                        format!("/build/{}", cache_path)
                    };
                    mounts.push(self.cache_mount(&absolute, cache_path));
                }

                let inputs = if i == 0 {
                    vec![base_idx, context_idx]
                } else {
                    vec![last_idx]
                };

                last_idx = self.create_exec(
                    inputs,
                    mounts,
                    meta,
                    Some(format!("Build command {}", i + 1)),
                );
                debug!("Build command {} created layer {}", i + 1, last_idx);
            }

            debug!("Build stage complete, build_result_idx={}", last_idx);
            last_idx
        } else {
            base_idx
        };

        // Stage 3: Runtime prep - Install runtime packages and remove apk
        let runtime_prep_idx = if !spec.runtime.packages.is_empty() {
            let packages = spec.runtime.packages.join(" ");

            // Install runtime packages
            let install_meta = pb::Meta {
                args: vec!["sh".to_string(), "-c".to_string(), format!("apk add --no-cache {}", packages)],
                env: vec![],
                cwd: "/".to_string(),
                user: String::new(),
                proxy_env: None,
                extra_hosts: vec![],
                hostname: String::new(),
                ulimit: vec![],
                cgroup_parent: String::new(),
                remove_mount_stubs_recursive: false,
                valid_exit_codes: vec![],
            };

            let install_mounts = vec![
                self.layer_mount(0, 0, "/"),  // Input 0: wolfi_base_idx
                self.scratch_mount("/tmp"),
            ];

            let runtime_with_packages_idx = self.create_exec(
                vec![wolfi_base_idx],
                install_mounts,
                install_meta,
                Some("Install runtime packages".to_string()),
            );

            // Remove apk tooling
            let cleanup_meta = pb::Meta {
                args: vec!["sh".to_string(), "-c".to_string(), "rm -rf /sbin/apk /etc/apk /lib/apk /var/cache/apk".to_string()],
                env: vec![],
                cwd: "/".to_string(),
                user: String::new(),
                proxy_env: None,
                extra_hosts: vec![],
                hostname: String::new(),
                ulimit: vec![],
                cgroup_parent: String::new(),
                remove_mount_stubs_recursive: false,
                valid_exit_codes: vec![],
            };

            let cleanup_mounts = vec![
                self.layer_mount(0, 0, "/"),  // Input 0: runtime_with_packages_idx
                self.scratch_mount("/tmp"),
            ];

            Some(self.create_exec(
                vec![runtime_with_packages_idx],
                cleanup_mounts,
                cleanup_meta,
                Some("Remove apk tooling".to_string()),
            ))
        } else {
            None
        };

        // Stage 4: Squash to glibc-dynamic base (using FileOp - glibc-dynamic has no shell)
        let squashed_idx = if let Some(runtime_prep) = runtime_prep_idx {
            debug!("Squash stage: glibc_dynamic_idx={}, runtime_prep={}", glibc_dynamic_idx, runtime_prep);
            let project_name = self.project_name.as_deref().unwrap_or("app");

            // Use FileOp to copy runtime_prep onto glibc-dynamic (no shell available in glibc-dynamic)
            Some(self.create_file_copy(
                glibc_dynamic_idx,
                runtime_prep,
                "/",
                "/",
                Some(format!("peelbox {} runtime", project_name)),
            ))
        } else {
            Some(glibc_dynamic_idx)
        };

        // Stage 5: Copy artifacts after squash using busybox as exec environment
        let final_idx = if let Some(squashed) = squashed_idx {
            if spec.runtime.copy.is_empty() {
                squashed
            } else {
                // Use busybox image as exec base with mounts for source and destination
                let mut current_idx = squashed;

                for copy in &spec.runtime.copy {
                    let src_path = if copy.from == "." {
                        "/artifacts-src/artifacts".to_string()
                    } else if copy.from.starts_with('/') {
                        let filename = std::path::Path::new(&copy.from)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(&copy.from);
                        format!("/artifacts-src/artifacts/{}", filename)
                    } else {
                        let filename = std::path::Path::new(&copy.from)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(&copy.from);
                        format!("/artifacts-src/artifacts/{}", filename)
                    };

                    let copy_cmd = format!(
                        "mkdir -p $(dirname {}) && cp -r {} {}",
                        copy.to, src_path, copy.to
                    );

                    let copy_meta = pb::Meta {
                        args: vec!["sh".to_string(), "-c".to_string(), copy_cmd],
                        env: vec![],
                        cwd: "/".to_string(),
                        user: String::new(),
                        proxy_env: None,
                        extra_hosts: vec![],
                        hostname: String::new(),
                        ulimit: vec![],
                        cgroup_parent: String::new(),
                        remove_mount_stubs_recursive: false,
                        valid_exit_codes: vec![],
                    };

                    // Use busybox as base (input 0), mount squashed at / (input 1, output 0), mount artifacts readonly (input 2)
                    let copy_mounts = vec![
                        self.layer_mount(1, 0, "/"),  // Input 1: current_idx (squashed) -> Output 0
                        self.readonly_mount(2, "/artifacts-src"),  // Input 2: build_result_idx (artifacts)
                        self.scratch_mount("/tmp"),
                    ];

                    debug!("ExecOp (busybox): copying from {} to {}", src_path, copy.to);
                    current_idx = self.create_exec(
                        vec![busybox_idx, current_idx, build_result_idx],
                        copy_mounts,
                        copy_meta,
                        Some(format!("Copy {} to {}", copy.from, copy.to)),
                    );
                }

                current_idx
            }
        } else {
            build_result_idx
        };

        // Add final reference-only node for provenance (required by BuildKit)
        let output_ref_idx = self.create_output_reference(final_idx);

        debug!("Built LLB graph with {} operations (final output: op {}, reference: op {})",
               self.ops.len(), final_idx, output_ref_idx);

        Ok(())
    }

    pub fn write_definition<W: Write>(&mut self, spec: &UniversalBuild, mut writer: W) -> Result<()> {
        let bytes = self.to_bytes(spec)?;
        writer.write_all(&bytes)?;
        Ok(())
    }
}
