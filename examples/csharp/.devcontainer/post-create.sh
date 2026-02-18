#!/bin/bash
# Post-create setup for C# example devcontainer

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

# Create local bin directory
mkdir -p ~/.local/bin

# Add local bin to PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc

# Restore NuGet packages
if [ -f "Angzarr.Examples.sln" ]; then
    dotnet restore
fi

echo ""
echo "C# development environment ready!"
echo "  - .NET 8.0 SDK"
echo "  - C# Dev Kit"
echo ""
echo "Quick start:"
echo "  just build        # Build the solution"
echo "  just test         # Run tests"
echo "  just              # List all commands"
