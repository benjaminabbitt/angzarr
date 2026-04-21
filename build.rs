use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Rerun if proto files or migration files change
    println!(
        "cargo:rerun-if-changed=angzarr-project/proto/angzarr_client/proto/angzarr/types.proto"
    );
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=angzarr-project/proto/angzarr_client/proto/angzarr/command_handler.proto");
    println!(
        "cargo:rerun-if-changed=angzarr-project/proto/angzarr_client/proto/angzarr/projector.proto"
    );
    println!(
        "cargo:rerun-if-changed=angzarr-project/proto/angzarr_client/proto/angzarr/saga.proto"
    );
    println!("cargo:rerun-if-changed=angzarr-project/proto/angzarr_client/proto/angzarr/process_manager.proto");
    println!(
        "cargo:rerun-if-changed=angzarr-project/proto/angzarr_client/proto/angzarr/query.proto"
    );
    println!(
        "cargo:rerun-if-changed=angzarr-project/proto/angzarr_client/proto/angzarr/stream.proto"
    );
    println!(
        "cargo:rerun-if-changed=angzarr-project/proto/angzarr_client/proto/angzarr/upcaster.proto"
    );
    println!(
        "cargo:rerun-if-changed=angzarr-project/proto/angzarr_client/proto/angzarr/meta.proto"
    );
    println!("cargo:rerun-if-changed=angzarr-project/proto/angzarr_client/proto/angzarr/cloudevents.proto");
    println!("cargo:rerun-if-changed=proto/io/cloudevents/v1/cloudevents.proto");

    // Generate descriptor.bin for proto reflection (used by COMMUTATIVE merge)
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let descriptor_path = out_dir.join("descriptor.bin");

    // Enable prost::Name trait for type reflection
    let mut prost_config = prost_build::Config::new();
    prost_config.enable_type_names();

    tonic_prost_build::configure()
        .file_descriptor_set_path(&descriptor_path)
        .build_server(true)
        .build_client(true)
        .type_attribute(
            ".angzarr_client.proto.angzarr.BusinessResponse.result",
            "#[allow(clippy::large_enum_variant)]",
        )
        .compile_with_config(
            prost_config,
            &[
                "angzarr-project/proto/angzarr_client/proto/angzarr/types.proto",
                "angzarr-project/proto/angzarr_client/proto/angzarr/command_handler.proto",
                "angzarr-project/proto/angzarr_client/proto/angzarr/projector.proto",
                "angzarr-project/proto/angzarr_client/proto/angzarr/saga.proto",
                "angzarr-project/proto/angzarr_client/proto/angzarr/process_manager.proto",
                "angzarr-project/proto/angzarr_client/proto/angzarr/query.proto",
                "angzarr-project/proto/angzarr_client/proto/angzarr/stream.proto",
                "angzarr-project/proto/angzarr_client/proto/angzarr/upcaster.proto",
                "angzarr-project/proto/angzarr_client/proto/angzarr/meta.proto",
                "angzarr-project/proto/angzarr_client/proto/angzarr/cloudevents.proto",
                "proto/io/cloudevents/v1/cloudevents.proto",
            ],
            &["angzarr-project/proto", "proto"],
        )?;
    Ok(())
}
