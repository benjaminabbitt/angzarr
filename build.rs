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
    println!("cargo:rerun-if-changed=proto/examples/inventory.proto");
    println!("cargo:rerun-if-changed=proto/examples/order.proto");
    println!("cargo:rerun-if-changed=proto/examples/fulfillment.proto");
    println!("cargo:rerun-if-changed=proto/examples/projections.proto");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .type_attribute(
            ".angzarr.BusinessResponse.result",
            "#[allow(clippy::large_enum_variant)]",
        )
        // Enable serde for descriptor types (K8s annotation serialization)
        .type_attribute(
            ".angzarr.ComponentDescriptor",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .type_attribute(
            ".angzarr.Target",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .compile_protos(
            &[
                "proto/angzarr/types.proto",
                "proto/angzarr/aggregate.proto",
                "proto/angzarr/projector.proto",
                "proto/angzarr/saga.proto",
                "proto/angzarr/process_manager.proto",
                "proto/angzarr/query.proto",
                "proto/angzarr/stream.proto",
                "proto/angzarr/upcaster.proto",
                "proto/examples/inventory.proto",
                "proto/examples/order.proto",
                "proto/examples/fulfillment.proto",
                "proto/examples/projections.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
