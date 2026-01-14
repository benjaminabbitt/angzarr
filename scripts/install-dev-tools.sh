#!/bin/bash
# Install development tools for angzarr-rs on Debian
# Uninstalls Docker and installs Podman, Kind, kubectl
# Run with: sudo ./scripts/install-dev-tools.sh

set -euo pipefail

echo "=========================================="
echo "  angzarr-rs Development Setup for Debian"
echo "=========================================="
echo ""

# === Check and Optionally Uninstall Docker ===
DOCKER_INSTALLED=false
DOCKER_IN_USE=false

if command -v docker &>/dev/null; then
    DOCKER_INSTALLED=true
    # Check for running containers
    if docker ps -q 2>/dev/null | grep -q .; then
        DOCKER_IN_USE=true
    fi
fi

if [ "$DOCKER_INSTALLED" = true ]; then
    if [ "$DOCKER_IN_USE" = true ]; then
        echo "=== Docker is in use (running containers detected) ==="
        echo "Skipping Docker removal. Stop containers first if you want to remove Docker."
        echo "Run: docker stop \$(docker ps -q) && sudo ./scripts/install-dev-tools.sh"
    else
        echo "=== Uninstalling Docker (not in use) ==="
        systemctl stop docker.socket docker.service 2>/dev/null || true
        systemctl disable docker.socket docker.service 2>/dev/null || true

        apt-get purge -y \
            docker-ce \
            docker-ce-cli \
            containerd.io \
            docker-buildx-plugin \
            docker-compose-plugin \
            docker.io \
            docker-compose \
            docker-doc \
            2>/dev/null || true

        rm -rf /var/lib/docker /var/lib/containerd /etc/docker 2>/dev/null || true
        groupdel docker 2>/dev/null || true

        echo "Docker removed"
    fi
else
    echo "=== Docker not installed, skipping removal ==="
fi

# === Install Podman ===
echo ""
echo "=== Installing Podman and podman-compose ==="
apt-get update
apt-get install -y podman podman-compose

# === Install Kind ===
echo ""
echo "=== Installing Kind ==="
KIND_VERSION=$(curl -s https://api.github.com/repos/kubernetes-sigs/kind/releases/latest | grep '"tag_name"' | cut -d'"' -f4)
curl -Lo /usr/local/bin/kind "https://kind.sigs.k8s.io/dl/${KIND_VERSION}/kind-linux-amd64"
chmod +x /usr/local/bin/kind
echo "Installed Kind ${KIND_VERSION}"

# === Install kubectl ===
echo ""
echo "=== Installing kubectl ==="
KUBECTL_VERSION=$(curl -L -s https://dl.k8s.io/release/stable.txt)
curl -LO "https://dl.k8s.io/release/${KUBECTL_VERSION}/bin/linux/amd64/kubectl"
install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl
rm kubectl
echo "Installed kubectl ${KUBECTL_VERSION}"

# === Cleanup ===
echo ""
echo "=== Cleaning up ==="
apt-get autoremove -y
apt-get autoclean

# === Verify installations ===
echo ""
echo "=== Verifying installations ==="
echo -n "podman:         "; podman --version
echo -n "podman-compose: "; podman-compose --version 2>/dev/null || podman-compose version
echo -n "kind:           "; kind --version
echo -n "kubectl:        "; kubectl version --client --short 2>/dev/null || kubectl version --client

echo ""
echo "=========================================="
echo "  Installation Complete!"
echo "=========================================="
echo ""
echo "Post-install steps (run as regular user):"
echo ""
echo "  1. Enable rootless podman socket:"
echo "     systemctl --user enable --now podman.socket"
echo ""
echo "  2. Add to ~/.bashrc or ~/.zshrc:"
echo "     export DOCKER_HOST=unix:///run/user/\$(id -u)/podman/podman.sock"
echo ""
echo "  3. Deploy to Kind cluster:"
echo "     just deploy"
echo ""
