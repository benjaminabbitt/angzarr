#!/bin/bash
# Generate Python protobuf files from evented.proto

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROTO_DIR="$SCRIPT_DIR/../../proto"
OUT_DIR="$SCRIPT_DIR"

echo "Generating Python protobuf files..."

# Generate proto
uv run python -m grpc_tools.protoc \
    -I"$PROTO_DIR" \
    --python_out="$OUT_DIR/evented/proto" \
    "$PROTO_DIR/evented/evented.proto"

# Move from nested evented/ to proto/
if [ -f "$OUT_DIR/evented/proto/evented/evented_pb2.py" ]; then
    mv "$OUT_DIR/evented/proto/evented/evented_pb2.py" "$OUT_DIR/evented/proto/"
    rm -rf "$OUT_DIR/evented/proto/evented"
fi

echo "Generated: $OUT_DIR/evented/proto/evented_pb2.py"
echo "Done!"
