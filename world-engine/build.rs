fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_path = std::path::Path::new("../protocol/a2a.proto");
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[proto_path], &["../protocol/"])?;
    Ok(())
}
