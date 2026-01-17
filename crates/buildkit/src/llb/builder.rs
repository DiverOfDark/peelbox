use crate::proto::pb;
use anyhow::Result;
use prost::Message as ProstMessage;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

pub struct LLBBuilder {
    pub(crate) context_name: String,
    pub(crate) context_path: Option<PathBuf>,
    pub(crate) project_name: Option<String>,
    pub(crate) session_id: Option<String>,

    pub(crate) ops: Vec<pb::Op>,
    pub(crate) digests: Vec<String>,
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

    pub(crate) fn add_op(&mut self, mut op: pb::Op) -> i64 {
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
        ProstMessage::encode(&op, &mut buf).expect("Failed to encode op");
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(&buf)));

        self.ops.push(op);
        self.digests.push(digest);

        index
    }

    pub(crate) fn get_cache_id(&self, cache_path: &str) -> String {
        let project_name = self.project_name.as_deref().unwrap_or("default");
        let normalized = cache_path.trim_start_matches("/build/").replace('/', "-");
        format!("{}-{}", project_name, normalized)
    }

    pub(crate) fn load_gitignore_patterns(&self) -> Vec<String> {
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

    pub(crate) fn create_merge(&mut self, inputs: Vec<(i64, i64)>) -> i64 {
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

    pub(crate) fn create_image_source(&mut self, image_ref: &str) -> i64 {
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

    pub(crate) fn create_output_reference(&mut self, input_idx: i64) -> i64 {
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

    pub(crate) fn create_local_source(&mut self, exclude_patterns: &[String]) -> i64 {
        let mut attrs = HashMap::new();

        if !exclude_patterns.is_empty() {
            attrs.insert("exclude-patterns".to_string(), exclude_patterns.join(","));
        }

        if let Some(path) = &self.context_path {
            if let Ok(hash) = self.calculate_context_hash(path) {
                attrs.insert("local.unique".to_string(), hash);
            }
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

    pub(crate) fn create_exec(
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

    pub(crate) fn cache_mount(&self, dest: &str, cache_path: &str) -> pb::Mount {
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

    pub(crate) fn layer_mount(&self, input_idx: i64, output_idx: i64, dest: &str) -> pb::Mount {
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

    pub(crate) fn readonly_mount(&self, input_idx: i64, dest: &str) -> pb::Mount {
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

    pub(crate) fn scratch_mount(&self, dest: &str) -> pb::Mount {
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

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.log_graph_structure();

        let mut def_bytes = Vec::new();
        for op in &self.ops {
            let mut op_bytes = Vec::new();
            ProstMessage::encode(op, &mut op_bytes)?;
            def_bytes.push(op_bytes);
        }

        let definition = pb::Definition {
            def: def_bytes,
            metadata: HashMap::new(),
            source: None,
        };

        let mut buf = Vec::new();
        ProstMessage::encode(&definition, &mut buf)?;

        Ok(buf)
    }

    fn log_graph_structure(&self) {
        let mut output = String::from("\n=== LLB Graph Structure ===\n\n");
        for (idx, op) in self.ops.iter().enumerate() {
            output.push_str(&format!(
                "{} ({}) -> ",
                idx,
                self.digests.get(idx).unwrap_or(&"".to_string())
            ));

            match &op.op {
                Some(pb::op::Op::Source(source)) => {
                    if source.identifier.starts_with("docker-image://") {
                        let image = source
                            .identifier
                            .strip_prefix("docker-image://")
                            .unwrap_or(&source.identifier);
                        output.push_str(&format!("FROM {}\n", image));
                    } else if source.identifier.starts_with("local://") {
                        let local = source
                            .identifier
                            .strip_prefix("local://")
                            .unwrap_or(&source.identifier);
                        output.push_str(&format!("FROM local://{}\n", local));
                    } else {
                        output.push_str(&format!("SOURCE {}\n", source.identifier));
                    }
                }
                Some(pb::op::Op::Exec(exec)) => {
                    if let Some(meta) = &exec.meta {
                        let args = meta.args.join(" ");
                        output.push_str(&format!("EXEC {}\n", args));
                    } else {
                        output.push_str("EXEC\n");
                    }

                    for mount in &exec.mounts {
                        let output_type = if mount.output >= 0 {
                            mount.output.to_string()
                        } else {
                            "-1".to_string()
                        };

                        output.push_str(&format!(
                            "          {} ({}) -> {} -> {}\n",
                            mount.input,
                            pb::MountType::try_from(mount.mount_type)
                                .unwrap()
                                .as_str_name(),
                            mount.dest,
                            output_type
                        ));
                    }
                }
                Some(pb::op::Op::Merge(merge)) => {
                    let inputs: Vec<String> =
                        merge.inputs.iter().map(|m| m.input.to_string()).collect();
                    output.push_str(&format!("MERGE ({})\n", inputs.join(", ")));
                }
                None => {
                    output.push_str("NONE\n");
                }
                _ => {
                    output.push_str("OTHER\n");
                }
            }

            for (i, input) in op.inputs.iter().enumerate() {
                output.push_str(&format!(
                    "          input[{}]: digest={}, index={}\n",
                    i, input.digest, input.index
                ));
            }
        }

        output.push_str("\n=== End of Graph ===\n");
        debug!("{}", output);
    }

    fn calculate_context_hash(&self, path: &Path) -> Result<String> {
        let mut hasher = Sha256::new();

        let entries: Vec<_> = ignore::WalkBuilder::new(path)
            .standard_filters(true)
            .hidden(false)
            .filter_entry(|e| {
                let path_str = e.path().to_string_lossy();
                !path_str.contains("/.git/")
            })
            .build()
            .enumerate()
            .filter_map(|(i, e)| match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    tracing::warn!("Failed to read context directory entry #{}: {}", i, err);
                    None
                }
            })
            .collect();

        for entry in entries {
            let entry_path = entry.path();
            let rel_path = entry_path.strip_prefix(path).unwrap_or(entry_path);
            hasher.update(rel_path.to_string_lossy().as_bytes());

            if let Some(file_type) = entry.file_type() {
                if file_type.is_file() {
                    match entry.metadata() {
                        Ok(metadata) => {
                            hasher.update(metadata.len().to_le_bytes());
                            if let Ok(mtime) = metadata.modified() {
                                if let Ok(duration) = mtime.duration_since(std::time::UNIX_EPOCH) {
                                    hasher.update(duration.as_secs().to_le_bytes());
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to read metadata for {:?}: {}", entry_path, e);
                        }
                    }
                }
            }
        }

        Ok(hex::encode(hasher.finalize()))
    }
}
