use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Rerun if proto files or migration files change
    println!("cargo:rerun-if-changed=proto/angzarr/types.proto");
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=proto/angzarr/aggregate.proto");
    println!("cargo:rerun-if-changed=proto/angzarr/projector.proto");
    println!("cargo:rerun-if-changed=proto/angzarr/saga.proto");
    println!("cargo:rerun-if-changed=proto/angzarr/process_manager.proto");
    println!("cargo:rerun-if-changed=proto/angzarr/query.proto");
    println!("cargo:rerun-if-changed=proto/angzarr/stream.proto");
    println!("cargo:rerun-if-changed=proto/angzarr/upcaster.proto");
    println!("cargo:rerun-if-changed=proto/angzarr/meta.proto");
    println!("cargo:rerun-if-changed=proto/angzarr/cloudevents.proto");
    println!("cargo:rerun-if-changed=proto/io/cloudevents/v1/cloudevents.proto");
    println!("cargo:rerun-if-changed=proto/examples/poker_types.proto");
    println!("cargo:rerun-if-changed=proto/examples/player.proto");
    println!("cargo:rerun-if-changed=proto/examples/table.proto");
    println!("cargo:rerun-if-changed=proto/examples/hand.proto");
    println!("cargo:rerun-if-changed=proto/examples/ai_sidecar.proto");

    // Generate descriptor.bin for proto reflection (used by COMMUTATIVE merge)
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let descriptor_path = out_dir.join("descriptor.bin");

    // Enable prost::Name trait for type reflection
    let mut prost_config = prost_build::Config::new();
    prost_config.enable_type_names();

    tonic_build::configure()
        .file_descriptor_set_path(&descriptor_path)
        .build_server(true)
        .build_client(true)
        .type_attribute(
            ".angzarr.BusinessResponse.result",
            "#[allow(clippy::large_enum_variant)]",
        )
        .compile_protos_with_config(
            prost_config,
            &[
                "proto/angzarr/types.proto",
                "proto/angzarr/aggregate.proto",
                "proto/angzarr/projector.proto",
                "proto/angzarr/saga.proto",
                "proto/angzarr/process_manager.proto",
                "proto/angzarr/query.proto",
                "proto/angzarr/stream.proto",
                "proto/angzarr/upcaster.proto",
                "proto/angzarr/meta.proto",
                "proto/angzarr/cloudevents.proto",
                "proto/io/cloudevents/v1/cloudevents.proto",
                "proto/examples/poker_types.proto",
                "proto/examples/player.proto",
                "proto/examples/table.proto",
                "proto/examples/hand.proto",
                "proto/examples/ai_sidecar.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
