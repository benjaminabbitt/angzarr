fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use main proto directory (../../proto relative to client/rust)
    // This ensures client uses the same protos as the server
    let proto_root = "../../proto";

    // Rerun if proto files change
    println!("cargo:rerun-if-changed={}", proto_root);

    let protos: Vec<String> = vec![
        format!("{}/angzarr/types.proto", proto_root),
        format!("{}/angzarr/aggregate.proto", proto_root),
        format!("{}/angzarr/projector.proto", proto_root),
        format!("{}/angzarr/saga.proto", proto_root),
        format!("{}/angzarr/process_manager.proto", proto_root),
        format!("{}/angzarr/query.proto", proto_root),
        format!("{}/angzarr/stream.proto", proto_root),
        format!("{}/angzarr/upcaster.proto", proto_root),
        format!("{}/angzarr/meta.proto", proto_root),
        format!("{}/angzarr/cloudevents.proto", proto_root),
        // Example protos (poker)
        format!("{}/examples/poker_types.proto", proto_root),
        format!("{}/examples/player.proto", proto_root),
        format!("{}/examples/table.proto", proto_root),
        format!("{}/examples/hand.proto", proto_root),
        format!("{}/examples/ai_sidecar.proto", proto_root),
    ];

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
        .compile_protos(&protos, &[proto_root])?;

    Ok(())
}
