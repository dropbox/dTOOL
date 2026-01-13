use std::io::Result;

fn main() -> Result<()> {
    // Configure tonic to generate gRPC client and server code
    tonic_build::configure()
        .type_attribute(".", "#[allow(clippy::large_enum_variant)]")
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_protos(&["../../proto/remote_node.proto"], &["../../proto/"])?;

    // Tell cargo to rerun this build script if the proto file changes
    println!("cargo:rerun-if-changed=../../proto/remote_node.proto");

    Ok(())
}
