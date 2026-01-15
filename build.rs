fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Only rerun if proto files change
    println!("cargo:rerun-if-changed=proto/angzarr/angzarr.proto");
    println!("cargo:rerun-if-changed=proto/examples/domains.proto");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/angzarr/angzarr.proto",
                "proto/examples/domains.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
