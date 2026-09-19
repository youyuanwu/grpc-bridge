fn main() {
    use std::path::PathBuf;

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=protos/helloworld.proto");

    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let tools_bin = manifest_dir
        .parent()
        .unwrap()
        .join(".tools/grpc-rust/bin");
    let executable_suffix = std::env::consts::EXE_SUFFIX;
    let protoc = tools_bin.join(format!("protoc{executable_suffix}"));
    let grpc_plugin = tools_bin.join(format!("protoc-gen-rust-grpc{executable_suffix}"));
    if !protoc.is_file() || !grpc_plugin.is_file() {
        panic!(
            "gRPC Rust codegen tools not found in {}. Run `cmake -P \
             cmake/install-grpc-rust-tools.cmake` from the repository root.",
            tools_bin.display()
        );
    }

    // Server-side stubs: standard tonic + prost.
    let mut prost_config = prost_build::Config::new();
    prost_config.protoc_executable(&protoc);
    tonic_prost_build::configure()
        .build_client(false)
        .compile_with_config(
            prost_config,
            &["protos/helloworld.proto"],
            &["protos"],
        )
        .unwrap();

    // Client-side stubs for the grpc-rust crate. grpc-protobuf-build emits
    // protobuf-rust messages (not prost) and gRPC client stubs that take a
    // `grpc::client::Channel`. Output is placed under a dedicated subdir of
    // OUT_DIR so the file names don't collide with the tonic codegen above.
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let grpc_out = out_dir.join("grpc_gen");
    std::fs::create_dir_all(&grpc_out).unwrap();
    grpc_protobuf_build::CodeGen::new()
        .prebuilt_binaries(&protoc, &grpc_plugin)
        .output_dir(&grpc_out)
        .input("helloworld.proto")
        .include("protos")
        .client_only()
        .compile()
        .unwrap();
}
