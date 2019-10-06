use protobuf_codegen_pure;

fn main() {

    // Main Tx model
    protobuf_codegen_pure::run(protobuf_codegen_pure::Args {
        out_dir: "src/",
        input: &[
            "proto/tx.proto",
        ],
        includes: &["proto"],
        customize: protobuf_codegen_pure::Customize {
            ..Default::default()
        },
    })
    .expect("protoc");

    // Protos used for test
    protobuf_codegen_pure::run(protobuf_codegen_pure::Args {
        out_dir: "tests/basic/cryptocurrency/",
        input: &[
            "tests/basic/cryptocurrency/proto/cryptocurrency.proto",
        ],
        includes: &["tests/basic/cryptocurrency/proto"],
        customize: protobuf_codegen_pure::Customize {
            ..Default::default()
        },
    })
    .expect("protoc");


}

