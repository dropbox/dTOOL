use std::io::Result;

fn main() -> Result<()> {
    // Configure prost to allow large enum variants in generated code
    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[allow(clippy::large_enum_variant)]");

    // Compile the protobuf schema
    config.compile_protos(&["../../proto/dashstream.proto"], &["../../proto/"])?;

    // Tell cargo to rerun this build script if the proto file changes
    println!("cargo:rerun-if-changed=../../proto/dashstream.proto");

    Ok(())
}
