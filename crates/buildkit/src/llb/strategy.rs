use super::builder::LLBBuilder;
use crate::proto::pb;
use anyhow::Result;
use peelbox_core::output::schema::UniversalBuild;

const WOLFI_BASE_IMAGE: &str = "cgr.dev/chainguard/wolfi-base:latest";
const SOURCE_DATE_EPOCH: &str = "0";

pub trait BuildStrategy {
    fn build_graph(&self, builder: &mut LLBBuilder, spec: &UniversalBuild) -> Result<()>;
}

pub struct PeelboxStrategy;

impl BuildStrategy for PeelboxStrategy {
    fn build_graph(&self, builder: &mut LLBBuilder, spec: &UniversalBuild) -> Result<()> {
        let wolfi_base_idx = builder.create_image_source(WOLFI_BASE_IMAGE);
        let glibc_dynamic_idx =
            builder.create_image_source("cgr.dev/chainguard/glibc-dynamic:latest");

        let exclude = builder.load_gitignore_patterns();
        let context_idx = builder.create_local_source(&exclude);

        // If build_image is set, use that image directly as the build base
        // (skipping Wolfi, apk add, and setup_commands). Used for languages
        // tightly coupled to specific C library versions (e.g., Swift).
        let custom_build_image_idx = spec
            .build
            .build_image
            .as_ref()
            .map(|image| builder.create_image_source(image));

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

        // When a custom build image is used, skip Wolfi packages and setup_commands entirely.
        let after_packages_idx = custom_build_image_idx
            .unwrap_or_else(|| with_build_packages_idx.unwrap_or(wolfi_base_idx));

        // Run setup commands on Wolfi AFTER apk add, BEFORE the build context is mounted.
        // Used for installing toolchains not available as Wolfi packages (e.g., Gleam).
        let base_idx = if custom_build_image_idx.is_some() {
            after_packages_idx
        } else if !spec.build.setup_commands.is_empty() {
            let script = spec.build.setup_commands.join(" && ");
            let meta = pb::Meta {
                args: vec!["sh".to_string(), "-c".to_string(), script],
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

            builder.create_exec(
                vec![(after_packages_idx, 0)],
                mounts,
                meta,
                Some("Run setup commands".to_string()),
            )
        } else {
            after_packages_idx
        };

        let build_result_idx = if !spec.build.commands.is_empty() {
            let mut last_idx = base_idx;

            for (i, command) in spec.build.commands.iter().enumerate() {
                let script = format!("cd /build && {}", command);

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

            // When busybox is in the runtime packages, create its applet symlinks
            // after installation. The --no-scripts flag skips post-install scripts,
            // so busybox symlinks (sh, env, ln, etc.) won't exist otherwise.
            // We use chroot so symlinks get correct absolute paths (e.g., /usr/bin/busybox
            // instead of /runtime-root/usr/bin/busybox).
            let busybox_symlinks = if spec.runtime.packages.iter().any(|p| p == "busybox") {
                " && chroot /runtime-root /usr/bin/busybox --install -s /usr/bin"
            } else {
                ""
            };

            let install_meta = pb::Meta {
                args: vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    format!("mkdir -p /runtime-root/etc/apk /runtime-root/var/lib/apk && cp -r /etc/apk/keys /runtime-root/etc/apk/ && echo \"https://packages.wolfi.dev/os\" > /runtime-root/etc/apk/repositories && apk add --root /runtime-root --no-cache --no-scripts --initdb {}{} && find /runtime-root -name \"*apk*\" -exec rm -rf {{}} +", packages, busybox_symlinks),
                ],
                env: vec![
                    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
                    format!("SOURCE_DATE_EPOCH={}", SOURCE_DATE_EPOCH),
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

        let mut merge_inputs = vec![(glibc_dynamic_idx, 0)];
        if let Some(pkg_idx) = runtime_packages_idx {
            merge_inputs.push((pkg_idx, 0));
        }

        // Create the base runtime layer (Glibc + Packages)
        let runtime_base_idx = builder.create_merge(merge_inputs);

        let final_image_idx = if !spec.runtime.copy.is_empty() {
            // TRANSFER EXEC OP
            // We run a shell command to copy artifacts from the build context (with caches mounted)
            // to the runtime base. This solves the "missing cached artifact" issue.

            // 1. Construct the copy script
            let mut copy_commands = Vec::new();
            for copy in &spec.runtime.copy {
                let mut src_path = if copy.from == "." {
                    "/build/.".to_string()
                } else if copy.from.starts_with('/') {
                    copy.from.clone()
                } else {
                    format!("/build/{}", copy.from)
                };

                // Ensure dest dir exists in /out
                // cp -a to preserve attributes, -r for recursion
                // We use /out as the root for destination
                let dest_path = format!("/out{}", copy.to);

                let is_dir_dest = copy.to.ends_with('/');

                if is_dir_dest {
                    copy_commands.push(format!("mkdir -p {}", dest_path));
                    // When source is a directory (ends with /), append "." to copy
                    // contents rather than the directory itself into dest
                    if src_path.ends_with('/') {
                        src_path.push('.');
                    }
                } else if let Some(parent) = std::path::Path::new(&dest_path).parent() {
                    copy_commands.push(format!("mkdir -p {}", parent.to_string_lossy()));
                }

                copy_commands.push(format!("cp -vra {} {}", src_path, dest_path));
            }
            let script = copy_commands.join(" && ");

            // 2. Setup Mounts
            // Input 0: Wolfi Base (Runner)
            // Input 1: Runtime Base (Target)
            // Input 2: Build Context (Source)

            let mut mounts = vec![
                builder.readonly_mount(0, "/temp"),
                builder.readonly_mount(1, "/"),
                builder.readonly_mount(2, "/build"),
                builder.layer_mount(3, 0, "/out"), // Runtime Base -> Output 0 (The Result)
            ];

            // Re-mount the SAME caches used in build to ensure artifacts are visible
            for cache_path in &spec.build.cache {
                let absolute: String = if cache_path.starts_with('/') {
                    cache_path.clone()
                } else {
                    format!("/build/{}", cache_path)
                };

                mounts.push(builder.cache_mount(&absolute, cache_path));
            }

            let meta = pb::Meta {
                args: vec!["sh".to_string(), "-c".to_string(), script],
                env: vec![format!("SOURCE_DATE_EPOCH={}", SOURCE_DATE_EPOCH)],
                cwd: "/".to_string(),
                user: String::new(),
                proxy_env: None,
                extra_hosts: vec![],
                hostname: String::new(),
                ulimit: vec![],
                cgroup_parent: String::new(),
                remove_mount_stubs_recursive: false,
            };

            // Inputs mapping
            // 0: Wolfi Base (provides shell for running copy commands)
            // 1: Runtime Base rootfs at / (target for copying to /out)
            // 2: Build source at /build (source for copying from)
            // 3: Runtime Base (cloned as output layer at /out)
            let inputs = vec![
                (wolfi_base_idx, 0),
                if !spec.build.commands.is_empty() {
                    (build_result_idx, 0) // Rootfs from build result
                } else {
                    (wolfi_base_idx, 0) // Wolfi base if no build commands
                },
                if !spec.build.commands.is_empty() {
                    (build_result_idx, 1) // Build artifacts from /build output
                } else {
                    (context_idx, 0) // Raw context if no build commands
                },
                (runtime_base_idx, 0),
            ];

            builder.create_exec(inputs, mounts, meta, Some("Transfer artifacts".to_string()))
        } else {
            runtime_base_idx
        };

        let _ = builder.create_output_reference(final_image_idx);
        Ok(())
    }
}
