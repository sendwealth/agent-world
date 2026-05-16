fn main() {
    let proto_path = "../protocol/a2a.proto";

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[proto_path], &["../protocol"])
        .expect("Failed to compile a2a.proto");
}
