fn main() {
    flatbuffers_tonic_build::compile_flatbuffers_tonic(&["./greeter.fbs"])
        .expect("flatbuffers tonic compilation failed");
}
