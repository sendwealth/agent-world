fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_path = "../protocol/world_engine.proto";

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[proto_path], &["../protocol/"])?;

    Ok(())
}
