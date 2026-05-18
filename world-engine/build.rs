use std::io::Result;

fn main() -> Result<()> {
    let proto_file = "../protocol/a2a.proto";
    println!("cargo:rerun-if-changed={}", proto_file);

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[proto_file], &["../protocol"])?;

    Ok(())
}
