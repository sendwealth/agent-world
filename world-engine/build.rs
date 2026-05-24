use std::io::Result;

fn main() -> Result<()> {
    let a2a_proto = "../protocol/a2a.proto";
    println!("cargo:rerun-if-changed={}", a2a_proto);

    let federation_proto = "../protocol/federation.proto";
    println!("cargo:rerun-if-changed={}", federation_proto);

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[a2a_proto, federation_proto], &["../protocol"])?;

    Ok(())
}
