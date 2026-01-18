#!/usr/bin/env bash
set -euo pipefail

# Install OpenTofu via standalone binary (avoids APT GPG issues on Trixie)

VERSION="${1:-1.8.8}"
INSTALL_DIR="/usr/local/bin"
TMP_DIR=$(mktemp -d)

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "Installing OpenTofu v${VERSION}..."

cd "$TMP_DIR"

FILENAME="tofu_${VERSION}_linux_amd64.zip"

# Download
curl -fsSL -o "$FILENAME" "https://github.com/opentofu/opentofu/releases/download/v${VERSION}/${FILENAME}"
curl -fsSL -o SHA256SUMS "https://github.com/opentofu/opentofu/releases/download/v${VERSION}/tofu_${VERSION}_SHA256SUMS"

# Verify checksum
EXPECTED=$(grep "$FILENAME" SHA256SUMS | awk '{print $1}')
ACTUAL=$(sha256sum "$FILENAME" | awk '{print $1}')

if [ "$EXPECTED" != "$ACTUAL" ]; then
    echo "Checksum verification failed!"
    echo "Expected: $EXPECTED"
    echo "Actual:   $ACTUAL"
    exit 1
fi
echo "Checksum verified."

# Extract and install
unzip -q "$FILENAME" tofu
install -m 755 tofu "$INSTALL_DIR/tofu"

echo "OpenTofu installed successfully:"
tofu --version
