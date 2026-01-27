# Proto generation commands

TOP := `git rev-parse --show-toplevel`

# Build the proto generation container
container-build:
    podman build -t angzarr-proto:latest "{{TOP}}/build/proto/"

# Generate all proto files using container
generate: container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --all

# Generate only Rust protos
rust: container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --rust
    @echo "Syncing generated Rust protos to examples/rust/common..."
    cp "{{TOP}}/generated/rust/examples/examples.rs" "{{TOP}}/examples/rust/common/src/proto/examples.rs"

# Generate only Python protos
python: container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --python

# Generate only Go protos
go: container-build
    podman run --rm \
        -v "{{TOP}}/proto:/workspace/proto:ro" \
        -v "{{TOP}}/generated:/workspace/generated" \
        angzarr-proto:latest --go

# Clean generated proto files
clean:
    rm -rf "{{TOP}}/generated"
