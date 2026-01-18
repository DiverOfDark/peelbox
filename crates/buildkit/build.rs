use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let proto_dir = PathBuf::from("proto");

    // Create proto directory if it doesn't exist
    fs::create_dir_all(&proto_dir)?;

    // Download proto files if they don't exist (for caching/committing)
    download_proto_if_missing(
        &proto_dir,
        "control.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.12.5/api/services/control/control.proto",
    )?;
    download_proto_if_missing(
        &proto_dir,
        "filesync.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.12.5/session/filesync/filesync.proto",
    )?;
    download_proto_if_missing(
        &proto_dir,
        "auth.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.12.5/session/auth/auth.proto",
    )?;

    download_proto_if_missing(
        &proto_dir,
        "ops.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.12.5/solver/pb/ops.proto",
    )?;
    download_proto_if_missing(
        &proto_dir,
        "worker.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.12.5/api/types/worker.proto",
    )?;
    download_proto_if_missing(
        &proto_dir,
        "policy.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.12.5/sourcepolicy/pb/policy.proto",
    )?;

    download_proto_if_missing(
        &proto_dir,
        "filesync.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.13.0/session/filesync/filesync.proto",
    )?;
    download_proto_if_missing(
        &proto_dir,
        "auth.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.13.0/session/auth/auth.proto",
    )?;

    download_proto_if_missing(
        &proto_dir,
        "ops.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.13.0/solver/pb/ops.proto",
    )?;

    // Exporter proto removed - 404 on GitHub and not required when enable_session_exporter=false

    download_proto_if_missing(
        &proto_dir,
        "ops.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.13.0/solver/pb/ops.proto",
    )?;
    download_proto_if_missing(
        &proto_dir,
        "worker.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.13.0/api/types/worker.proto",
    )?;
    download_proto_if_missing(
        &proto_dir,
        "policy.proto",
        "https://raw.githubusercontent.com/moby/buildkit/v0.13.0/sourcepolicy/pb/policy.proto",
    )?;
    download_proto_if_missing(
        &proto_dir,
        "wire.proto",
        "https://raw.githubusercontent.com/tonistiigi/fsutil/master/types/wire.proto",
    )?;
    download_proto_if_missing(
        &proto_dir,
        "stat.proto",
        "https://raw.githubusercontent.com/tonistiigi/fsutil/master/types/stat.proto",
    )?;

    // Download containerd content store proto
    download_proto_if_missing(
        &proto_dir,
        "content.proto",
        "https://raw.githubusercontent.com/containerd/containerd/v1.7.13/api/services/content/v1/content.proto",
    )?;

    // Download Google well-known types
    let google_dir = proto_dir.join("google").join("protobuf");
    fs::create_dir_all(&google_dir)?;
    download_proto_if_missing(&google_dir, "timestamp.proto",
        "https://raw.githubusercontent.com/protocolbuffers/protobuf/main/src/google/protobuf/timestamp.proto")?;
    download_proto_if_missing(&google_dir, "duration.proto",
        "https://raw.githubusercontent.com/protocolbuffers/protobuf/main/src/google/protobuf/duration.proto")?;
    download_proto_if_missing(&google_dir, "any.proto",
        "https://raw.githubusercontent.com/protocolbuffers/protobuf/main/src/google/protobuf/any.proto")?;
    download_proto_if_missing(&google_dir, "empty.proto",
        "https://raw.githubusercontent.com/protocolbuffers/protobuf/main/src/google/protobuf/empty.proto")?;
    download_proto_if_missing(&google_dir, "descriptor.proto",
        "https://raw.githubusercontent.com/protocolbuffers/protobuf/main/src/google/protobuf/descriptor.proto")?;
    download_proto_if_missing(&google_dir, "field_mask.proto",
        "https://raw.githubusercontent.com/protocolbuffers/protobuf/main/src/google/protobuf/field_mask.proto")?;

    let google_rpc_dir = proto_dir.join("google").join("rpc");
    fs::create_dir_all(&google_rpc_dir)?;
    download_proto_if_missing(
        &google_rpc_dir,
        "status.proto",
        "https://raw.githubusercontent.com/googleapis/googleapis/master/google/rpc/status.proto",
    )?;

    let gogo_dir = proto_dir
        .join("github.com")
        .join("gogo")
        .join("protobuf")
        .join("gogoproto");
    fs::create_dir_all(&gogo_dir)?;
    download_proto_if_missing(
        &gogo_dir,
        "gogo.proto",
        "https://raw.githubusercontent.com/gogo/protobuf/master/gogoproto/gogo.proto",
    )?;

    // Create processed versions of proto files with fixed import paths
    let processed_dir = out_dir.join("proto_processed");

    fs::create_dir_all(&processed_dir)?;

    process_proto_file(
        &proto_dir,
        &processed_dir,
        "control.proto",
        &[
            (
                "github.com/moby/buildkit/api/types/worker.proto",
                "worker.proto",
            ),
            ("github.com/moby/buildkit/solver/pb/ops.proto", "ops.proto"),
            (
                "github.com/moby/buildkit/sourcepolicy/pb/policy.proto",
                "policy.proto",
            ),
            (
                "github.com/gogo/googleapis/google/rpc/status.proto",
                "google/rpc/status.proto",
            ),
        ],
    )?;

    process_proto_file(
        &proto_dir,
        &processed_dir,
        "filesync.proto",
        &[(
            "github.com/tonistiigi/fsutil/types/wire.proto",
            "wire.proto",
        )],
    )?;

    process_proto_file(&proto_dir, &processed_dir, "auth.proto", &[])?;

    process_proto_file(&proto_dir, &processed_dir, "ops.proto", &[])?;

    process_proto_file(
        &proto_dir,
        &processed_dir,
        "worker.proto",
        &[("github.com/moby/buildkit/solver/pb/ops.proto", "ops.proto")],
    )?;

    process_proto_file(&proto_dir, &processed_dir, "policy.proto", &[])?;

    process_proto_file(
        &proto_dir,
        &processed_dir,
        "wire.proto",
        &[
            (
                "github.com/tonistiigi/fsutil/types/stat.proto",
                "stat.proto",
            ),
            ("github.com/planetscale/vtprotobuf/vtproto/ext.proto", ""), // Remove this import
        ],
    )?;

    process_proto_file(
        &proto_dir,
        &processed_dir,
        "stat.proto",
        &[
            ("github.com/planetscale/vtprotobuf/vtproto/ext.proto", ""), // Remove this import
        ],
    )?;

    process_proto_file(
        &proto_dir,
        &processed_dir,
        "content.proto",
        &[], // No import path replacements needed
    )?;

    // Copy Google proto files to processed directory (no replacements needed)
    let processed_google_dir = processed_dir.join("google").join("protobuf");
    fs::create_dir_all(&processed_google_dir)?;
    for file in &[
        "timestamp.proto",
        "duration.proto",
        "any.proto",
        "empty.proto",
        "descriptor.proto",
        "field_mask.proto",
    ] {
        fs::copy(google_dir.join(file), processed_google_dir.join(file))?;
    }

    let processed_google_rpc_dir = processed_dir.join("google").join("rpc");
    fs::create_dir_all(&processed_google_rpc_dir)?;
    fs::copy(
        google_rpc_dir.join("status.proto"),
        processed_google_rpc_dir.join("status.proto"),
    )?;

    let processed_gogo_dir = processed_dir
        .join("github.com")
        .join("gogo")
        .join("protobuf")
        .join("gogoproto");
    fs::create_dir_all(&processed_gogo_dir)?;
    fs::copy(
        gogo_dir.join("gogo.proto"),
        processed_gogo_dir.join("gogo.proto"),
    )?;

    // Generate Rust code from proto files
    tonic_build::configure()
        .build_server(true) // We need server for FileSync and Content
        .build_client(true)
        .out_dir(&out_dir)
        .compile_protos(
            &[
                processed_dir.join("control.proto"),
                processed_dir.join("filesync.proto"),
                processed_dir.join("auth.proto"),
                processed_dir.join("content.proto"),
            ],
            std::slice::from_ref(&processed_dir),
        )?;

    println!("cargo:rerun-if-changed=proto/");
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}

fn download_proto_if_missing(
    proto_dir: &std::path::Path,
    filename: &str,
    url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = proto_dir.join(filename);

    if file_path.exists() {
        println!("Proto file {} already exists, skipping download", filename);
        return Ok(());
    }

    println!("Downloading {} from {}", filename, url);

    let content = ureq::get(url).call()?.into_string()?;

    fs::write(&file_path, content)?;
    println!("Downloaded {} successfully", filename);

    Ok(())
}

fn process_proto_file(
    source_dir: &std::path::Path,
    dest_dir: &std::path::Path,
    filename: &str,
    replacements: &[(&str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = source_dir.join(filename);
    let dest_path = dest_dir.join(filename);

    let mut content = fs::read_to_string(source_path)?;

    for (old, new) in replacements {
        if new.is_empty() {
            // Remove import entirely
            let import_line = format!("import \"{}\";", old);
            content = content.replace(&import_line, &format!("// {} (removed)", import_line));
        } else {
            // Replace import path
            content = content.replace(old, new);
        }
    }

    // Remove vtproto option lines
    content = content
        .lines()
        .filter(|line| !line.trim().starts_with("option (vtproto."))
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(dest_path, content)?;
    Ok(())
}
