#!/usr/bin/env bash
# Load podman images into kind cluster
# Usage: kind-load-images.sh <cluster-name> <image1:tag> [image2:tag ...]

set -euo pipefail

CLUSTER_NAME="${1:?Usage: $0 <cluster-name> <image1:tag> [image2:tag ...]}"
shift

if [[ $# -eq 0 ]]; then
    echo "Error: No images specified" >&2
    exit 1
fi

TMPDIR="${TMPDIR:-/tmp}"
export KIND_EXPERIMENTAL_PROVIDER=podman

for IMAGE in "$@"; do
    ARCHIVE="${TMPDIR}/kind-load-$(echo "$IMAGE" | tr ':/' '-').tar"
    echo "Loading ${IMAGE} into kind cluster ${CLUSTER_NAME}..."
    podman save "$IMAGE" -o "$ARCHIVE"
    kind load image-archive "$ARCHIVE" --name "$CLUSTER_NAME"
    rm -f "$ARCHIVE"
done

echo "All images loaded successfully"
