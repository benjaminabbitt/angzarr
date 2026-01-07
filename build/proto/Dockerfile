# Protobuf generation container
# Generates proto files for Rust, Python, and Go

FROM golang:1.22-bookworm AS base

# Install protoc and base dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    python3 \
    python3-pip \
    python3-venv \
    unzip \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Go protoc plugins
RUN go install google.golang.org/protobuf/cmd/protoc-gen-go@latest && \
    go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@latest

# Install Python grpcio-tools
RUN python3 -m venv /opt/venv && \
    /opt/venv/bin/pip install --no-cache-dir grpcio-tools

# Install Rust (for tonic/prost generation)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install protoc-gen-prost plugins
RUN cargo install protoc-gen-prost protoc-gen-tonic protoc-gen-prost-crate

# Set up paths
ENV PATH="/go/bin:/opt/venv/bin:${PATH}"

WORKDIR /workspace

# Copy the generation script
COPY generate.sh /usr/local/bin/generate-protos
RUN chmod +x /usr/local/bin/generate-protos

ENTRYPOINT ["/usr/local/bin/generate-protos"]
