use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/rs.proto")?;

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("rs_descriptor.bin"))
        .compile_protos(&["proto/rs.proto"], &["proto"])
        .unwrap();

    Ok(())
}
