#!/bin/bash
# Post-create setup for Python example devcontainer

set -e

# Install system dependencies
sudo apt-get update && sudo apt-get install -y --no-install-recommends \
    protobuf-compiler \
    libprotobuf-dev \
    jq \
    && sudo rm -rf /var/lib/apt/lists/*

# Install just
curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to ~/.local/bin

# Install grpcurl
GRPCURL_VERSION=1.9.1
curl -sSL "https://github.com/fullstorydev/grpcurl/releases/download/v${GRPCURL_VERSION}/grpcurl_${GRPCURL_VERSION}_linux_x86_64.tar.gz" \
    | sudo tar --no-same-owner -xzf - -C /usr/local/bin grpcurl

# Install kind
sudo curl -Lo /usr/local/bin/kind https://kind.sigs.k8s.io/dl/v0.20.0/kind-linux-amd64 \
    && sudo chmod +x /usr/local/bin/kind

# Install uv (fast Python package manager)
curl -LsSf https://astral.sh/uv/install.sh | sh

# Create local bin directory
mkdir -p ~/.local/bin

# Add local bin to PATH
echo 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"' >> ~/.bashrc

# Set up Python environment
if [ -f "pyproject.toml" ]; then
    ~/.cargo/bin/uv sync
fi

echo ""
echo "Python development environment ready!"
echo "  - uv: fast package manager"
echo "  - ruff: linting and formatting"
echo ""
echo "Quick start:"
echo "  just build        # Build/sync dependencies"
echo "  just test         # Run tests"
echo "  just              # List all commands"
