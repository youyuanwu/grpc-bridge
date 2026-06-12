use tonic_rest_build::{RestCodegenConfig, dump_file_descriptor_set, generate};

const PROTO_FILES: &[&str] = &["proto/greeter.proto"];
const PROTO_INCLUDES: &[&str] = &["proto"];

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let descriptor_path = format!("{out_dir}/file_descriptor_set.bin");

    let descriptor_bytes = dump_file_descriptor_set(PROTO_FILES, PROTO_INCLUDES, &descriptor_path);

    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .type_attribute(
            ".",
            "#[derive(serde::Serialize, serde::Deserialize)] #[serde(default)]",
        )
        .compile_protos(PROTO_FILES, PROTO_INCLUDES)
        .expect("tonic-prost-build compile_protos failed");

    let rest_config = RestCodegenConfig::new();
    let code = generate(&descriptor_bytes, &rest_config).expect("tonic-rest-build generate failed");
    std::fs::write(format!("{out_dir}/rest_routes.rs"), code).unwrap();

    for f in PROTO_FILES {
        println!("cargo:rerun-if-changed={f}");
    }
}
