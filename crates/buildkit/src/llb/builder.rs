use crate::proto::pb;
use anyhow::Result;
use prost::Message as ProstMessage;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

pub struct LLBBuilder {
    pub(crate) context_name: String,
    pub(crate) context_path: Option<PathBuf>,
    pub(crate) project_name: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) cache_namespace: Option<String>,

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
            cache_namespace: None,
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

    pub fn with_cache_namespace(mut self, ns: String) -> Self {
        self.cache_namespace = Some(ns);
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

    pub fn get_cache_id(&self, cache_path: &str) -> String {
        let project_name = self.project_name.as_deref().unwrap_or_else(|| {
            // Generate a stable random UUID for this instance if no project name provided
            // This prevents cache sharing between unnamed projects
            static DEFAULT_PROJECT_ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            static WARN_LOGGED: std::sync::OnceLock<()> = std::sync::OnceLock::new();

            let id = DEFAULT_PROJECT_ID.get_or_init(|| {
                let new_id = uuid::Uuid::new_v4().to_string();
                // Log warning only once when ID is first initialized
                WARN_LOGGED.get_or_init(|| {
                    tracing::warn!(
                        "No project name provided, using transient cache ID: {}",
                        new_id
                    );
                });
                new_id
            });
            id
        });
        let normalized = cache_path.trim_start_matches("/build/").replace('/', "-");
        if let Some(ns) = &self.cache_namespace {
            let prefix = &ns[..ns.len().min(12)];
            format!("{}-{}-{}", project_name, prefix, normalized)
        } else {
            format!("{}-{}", project_name, normalized)
        }
    }

    pub(crate) fn load_gitignore_patterns(&self) -> Vec<String> {
        let context_root = self
            .context_path
            .as_deref()
            .unwrap_or_else(|| Path::new("."));
        load_exclude_patterns(context_root)
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
            if let Ok(hash) = self.calculate_context_hash(path, exclude_patterns) {
                info!("local.unique context hash: {}", hash);
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
        info!("{}", output);
    }

    fn calculate_context_hash(&self, path: &Path, exclude_patterns: &[String]) -> Result<String> {
        calculate_context_hash(path, exclude_patterns)
    }
}

/// Calculate a stable content hash for the build context directory.
/// Uses the same exclude-pattern logic as `FileSync::scan_files` so the hash
/// covers exactly the files that BuildKit will receive via DiffCopy.
pub fn calculate_context_hash(path: &Path, exclude_patterns: &[String]) -> Result<String> {
    let mut hasher = Sha256::new();

    let mut overrides = ignore::overrides::OverrideBuilder::new(path);
    for pattern in exclude_patterns {
        let negated = format!("!{}", pattern);
        if let Err(e) = overrides.add(&negated) {
            tracing::warn!("Failed to add exclude pattern '{}': {}", pattern, e);
        }
    }
    let overrides = overrides.build().unwrap_or_else(|_| {
        ignore::overrides::OverrideBuilder::new(path)
            .build()
            .unwrap()
    });

    let mut entries: Vec<_> = ignore::WalkBuilder::new(path)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .overrides(overrides)
        .filter_entry(|e| !e.path().to_string_lossy().contains("/.git/"))
        .build()
        .filter_map(|e| match e {
            Ok(entry) => {
                // Skip root directory
                if entry.path() == path {
                    None
                } else {
                    Some(entry)
                }
            }
            Err(err) => {
                tracing::warn!("Failed to read context directory entry: {}", err);
                None
            }
        })
        .collect();

    entries.sort_by(|a, b| a.path().cmp(b.path()));

    for entry in entries {
        let entry_path = entry.path();
        let rel_path = entry_path.strip_prefix(path).unwrap_or(entry_path);
        hasher.update(rel_path.to_string_lossy().as_bytes());

        if let Some(file_type) = entry.file_type() {
            if file_type.is_file() {
                match entry.metadata() {
                    Ok(metadata) => {
                        hasher.update(metadata.len().to_le_bytes());

                        if let Ok(content) = fs::read(entry_path) {
                            hasher.update(&content);
                        } else {
                            tracing::warn!(
                                "Failed to read file content for hashing: {:?}",
                                entry_path
                            );
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

/// Build the list of exclude patterns for the build context.
/// Reads `.gitignore` from `context_root` and appends hard-coded patterns
/// for files that should never be part of a build context.
pub fn load_exclude_patterns(context_root: &Path) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_context_hash_stable_when_excluded_files_change() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("main.rs"), b"fn main() {}").unwrap();
        fs::write(temp_dir.path().join("README.md"), b"# v1").unwrap();

        let builder = LLBBuilder::new("context").with_context_path(temp_dir.path().to_path_buf());
        let exclude = vec!["*.md".to_string()];

        let hash1 = builder
            .calculate_context_hash(temp_dir.path(), &exclude)
            .unwrap();

        // Change excluded file — hash should NOT change
        fs::write(
            temp_dir.path().join("README.md"),
            b"# v2 completely different",
        )
        .unwrap();
        let hash2 = builder
            .calculate_context_hash(temp_dir.path(), &exclude)
            .unwrap();

        assert_eq!(
            hash1, hash2,
            "Hash must be stable when only excluded files change"
        );
    }

    #[test]
    fn test_context_hash_changes_when_included_files_change() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("main.rs"), b"fn main() {}").unwrap();

        let builder = LLBBuilder::new("context").with_context_path(temp_dir.path().to_path_buf());
        let exclude = vec!["*.md".to_string()];

        let hash1 = builder
            .calculate_context_hash(temp_dir.path(), &exclude)
            .unwrap();

        // Change included file — hash MUST change
        fs::write(
            temp_dir.path().join("main.rs"),
            b"fn main() { println!(\"hi\"); }",
        )
        .unwrap();
        let hash2 = builder
            .calculate_context_hash(temp_dir.path(), &exclude)
            .unwrap();

        assert_ne!(hash1, hash2, "Hash must change when included files change");
    }

    #[test]
    fn test_load_exclude_patterns_includes_hardcoded() {
        let temp_dir = TempDir::new().unwrap();
        let patterns = load_exclude_patterns(temp_dir.path());
        assert!(patterns.contains(&"*.md".to_string()));
        assert!(patterns.contains(&".vscode/".to_string()));
        assert!(patterns.contains(&".idea/".to_string()));
        assert!(patterns.contains(&"*.tar".to_string()));
    }
}
