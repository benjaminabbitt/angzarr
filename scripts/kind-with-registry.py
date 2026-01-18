#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["pyyaml"]
# ///
"""Create a kind cluster with a local container registry.

Creates a kind cluster configured to use a local registry for faster
image pulls. Uses podman as the container runtime.

Based on: https://github.com/bkuzmic/skaffold-podman-kind
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class PortMapping:
    """Port mapping from host to container."""

    container_port: int
    host_port: int
    comment: str
    protocol: str = "TCP"


@dataclass
class Config:
    """Cluster configuration."""

    cluster_name: str = "angzarr"
    registry_name: str = "kind-registry"
    registry_port: int = 5001
    # Use k8s 1.31 for better rootless podman compatibility (1.35 has issues)
    node_image: str = "kindest/node:v1.31.4"

    port_mappings: list[PortMapping] = field(default_factory=lambda: [
        PortMapping(80, 8080, "Ingress HTTP"),
        PortMapping(443, 8443, "Ingress HTTPS"),
        PortMapping(30051, 50051, "Angzarr command handler"),
        PortMapping(30052, 50052, "Angzarr event query"),
        PortMapping(30053, 50053, "Angzarr proxy"),
        PortMapping(30054, 50054, "Angzarr stream"),
        PortMapping(30672, 5672, "RabbitMQ AMQP"),
        PortMapping(31672, 15672, "RabbitMQ Management"),
        PortMapping(30379, 6379, "Redis"),
    ])


def run(
    *args: str,
    capture: bool = False,
    check: bool = True,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess:
    """Run a command."""
    full_env = {**os.environ, **(env or {})}
    if capture:
        return subprocess.run(
            args, capture_output=True, text=True, check=check, env=full_env
        )
    return subprocess.run(args, check=check, env=full_env)


def podman(*args: str, capture: bool = False, check: bool = True) -> subprocess.CompletedProcess:
    """Run podman command."""
    return run("podman", *args, capture=capture, check=check)


def kind(*args: str, capture: bool = False, check: bool = True, use_scope: bool = False) -> subprocess.CompletedProcess:
    """Run kind command with podman provider.

    Args:
        use_scope: Wrap with systemd-run --user --scope for rootless cgroup delegation.
    """
    env = {"KIND_EXPERIMENTAL_PROVIDER": "podman"}
    if use_scope and os.getuid() != 0:
        return run("systemd-run", "--user", "--scope", "kind", *args, capture=capture, check=check, env=env)
    return run("kind", *args, capture=capture, check=check, env=env)


def kubectl(*args: str, capture: bool = False, check: bool = True) -> subprocess.CompletedProcess:
    """Run kubectl command."""
    return run("kubectl", *args, capture=capture, check=check)


def cluster_exists(name: str) -> bool:
    """Check if a kind cluster exists."""
    result = kind("get", "clusters", capture=True, check=False)
    if result.returncode != 0:
        return False
    return name in result.stdout.strip().split("\n")


def registry_exists(name: str) -> bool:
    """Check if the registry container exists."""
    result = podman("container", "inspect", name, capture=True, check=False)
    return result.returncode == 0


def registry_running(name: str) -> bool:
    """Check if the registry container is running."""
    result = podman(
        "container", "inspect", name,
        "--format", "{{.State.Running}}",
        capture=True, check=False
    )
    return result.returncode == 0 and result.stdout.strip() == "true"


def ensure_volume(name: str) -> None:
    """Ensure a podman volume exists."""
    result = podman("volume", "inspect", name, capture=True, check=False)
    if result.returncode != 0:
        print(f"Creating persistent volume '{name}'...")
        podman("volume", "create", name)
    else:
        print(f"Volume '{name}' already exists")


def ensure_registry(cfg: Config) -> None:
    """Ensure the registry container is running with persistent storage."""
    volume_name = f"{cfg.registry_name}-data"
    ensure_volume(volume_name)

    if not registry_exists(cfg.registry_name):
        print(f"Creating local registry on port {cfg.registry_port}...")
        podman(
            "run", "-d",
            "--restart=always",
            "-p", f"127.0.0.1:{cfg.registry_port}:5000",
            "-v", f"{volume_name}:/var/lib/registry",
            "-e", "REGISTRY_STORAGE_DELETE_ENABLED=true",
            "--name", cfg.registry_name,
            "docker.io/library/registry:2",
        )
    elif not registry_running(cfg.registry_name):
        print("Starting stopped registry...")
        podman("start", cfg.registry_name)
    else:
        print("Registry already running")

    # Connect to kind network (ignore errors if already connected or network doesn't exist)
    podman("network", "connect", "kind", cfg.registry_name, check=False, capture=True)


def generate_kind_config(cfg: Config) -> str:
    """Generate kind cluster configuration YAML."""
    port_mappings_yaml = "\n".join(
        f"""      # {pm.comment}
      - containerPort: {pm.container_port}
        hostPort: {pm.host_port}
        protocol: {pm.protocol}"""
        for pm in cfg.port_mappings
    )

    return f"""kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
containerdConfigPatches:
  - |-
    [plugins."io.containerd.grpc.v1.cri".registry.mirrors."localhost:{cfg.registry_port}"]
      endpoint = ["http://{cfg.registry_name}:5000"]
nodes:
  - role: control-plane
    kubeadmConfigPatches:
      - |
        kind: InitConfiguration
        nodeRegistration:
          kubeletExtraArgs:
            node-labels: "ingress-ready=true"
    extraPortMappings:
{port_mappings_yaml}
"""


def create_registry_configmap(cfg: Config) -> None:
    """Create ConfigMap documenting the local registry."""
    configmap_yaml = f"""apiVersion: v1
kind: ConfigMap
metadata:
  name: local-registry-hosting
  namespace: kube-public
data:
  localRegistryHosting.v1: |
    host: "localhost:{cfg.registry_port}"
    help: "https://kind.sigs.k8s.io/docs/user/local-registry/"
"""
    result = subprocess.run(
        ["kubectl", "apply", "-f", "-"],
        input=configmap_yaml,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        print(f"Warning: Failed to create registry ConfigMap: {result.stderr}", file=sys.stderr)


def create_cluster(cfg: Config) -> None:
    """Create the kind cluster."""
    # Generate config
    config_yaml = generate_kind_config(cfg)

    # Write to temp file
    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(config_yaml)
        config_path = f.name

    try:
        print(f"Creating kind cluster '{cfg.cluster_name}' with {cfg.node_image}...")
        # Use systemd scope for rootless podman cgroup delegation
        kind("create", "cluster", "--name", cfg.cluster_name, "--config", config_path,
             "--image", cfg.node_image, use_scope=True)
    finally:
        Path(config_path).unlink(missing_ok=True)

    # Connect registry to kind network
    print("Connecting registry to kind network...")
    podman("network", "connect", "kind", cfg.registry_name, check=False, capture=True)

    # Create ConfigMap
    create_registry_configmap(cfg)

    print()
    print(f"Cluster '{cfg.cluster_name}' created with local registry at localhost:{cfg.registry_port}")
    print()
    print("Configure podman to trust the registry by adding to ~/.config/containers/registries.conf:")
    print()
    print("[[registry]]")
    print(f'location="localhost:{cfg.registry_port}"')
    print("insecure=true")
    print()
    print("Configure skaffold by adding to ~/.skaffold/config:")
    print()
    print("global:")
    print("  kind-disable-load: true")


def delete_cluster(cfg: Config) -> None:
    """Delete the kind cluster."""
    if not cluster_exists(cfg.cluster_name):
        print(f"Cluster '{cfg.cluster_name}' does not exist")
        return

    print(f"Deleting kind cluster '{cfg.cluster_name}'...")
    kind("delete", "cluster", "--name", cfg.cluster_name)
    print(f"Cluster '{cfg.cluster_name}' deleted")


def delete_registry(cfg: Config) -> None:
    """Delete the registry container."""
    if not registry_exists(cfg.registry_name):
        print(f"Registry '{cfg.registry_name}' does not exist")
        return

    print(f"Stopping and removing registry '{cfg.registry_name}'...")
    podman("stop", cfg.registry_name, check=False)
    podman("rm", cfg.registry_name, check=False)
    print(f"Registry '{cfg.registry_name}' deleted")


def status(cfg: Config) -> None:
    """Show status of cluster and registry."""
    print("=== Cluster Status ===")
    if cluster_exists(cfg.cluster_name):
        print(f"Cluster '{cfg.cluster_name}': EXISTS")
        kubectl("cluster-info", "--context", f"kind-{cfg.cluster_name}", check=False)
    else:
        print(f"Cluster '{cfg.cluster_name}': NOT FOUND")

    print()
    print("=== Registry Status ===")
    if registry_exists(cfg.registry_name):
        running = registry_running(cfg.registry_name)
        state = "RUNNING" if running else "STOPPED"
        print(f"Registry '{cfg.registry_name}': {state}")
        if running:
            # Test registry connectivity
            import urllib.request
            try:
                with urllib.request.urlopen(f"http://localhost:{cfg.registry_port}/v2/", timeout=2) as resp:
                    if resp.status == 200:
                        print(f"Registry API: OK (http://localhost:{cfg.registry_port}/v2/)")
            except Exception as e:
                print(f"Registry API: UNREACHABLE ({e})")
    else:
        print(f"Registry '{cfg.registry_name}': NOT FOUND")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Create a kind cluster with a local container registry",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s              Create cluster and registry (default)
  %(prog)s create       Create cluster and registry
  %(prog)s delete       Delete cluster (keeps registry)
  %(prog)s delete-all   Delete cluster and registry
  %(prog)s status       Show cluster and registry status
        """,
    )

    parser.add_argument(
        "command",
        nargs="?",
        default="create",
        choices=["create", "delete", "delete-all", "status"],
        help="Command to run (default: create)",
    )
    parser.add_argument(
        "--cluster-name",
        default=os.environ.get("CLUSTER_NAME", "angzarr"),
        help="Kind cluster name (default: angzarr, or CLUSTER_NAME env var)",
    )
    parser.add_argument(
        "--registry-name",
        default=os.environ.get("REGISTRY_NAME", "kind-registry"),
        help="Registry container name (default: kind-registry, or REGISTRY_NAME env var)",
    )
    parser.add_argument(
        "--registry-port",
        type=int,
        default=int(os.environ.get("REGISTRY_PORT", "5001")),
        help="Registry port (default: 5001, or REGISTRY_PORT env var)",
    )
    parser.add_argument(
        "--node-image",
        default=os.environ.get("KIND_NODE_IMAGE", "kindest/node:v1.31.4"),
        help="Kubernetes node image (default: kindest/node:v1.31.4, or KIND_NODE_IMAGE env var)",
    )

    args = parser.parse_args()

    cfg = Config(
        cluster_name=args.cluster_name,
        registry_name=args.registry_name,
        registry_port=args.registry_port,
        node_image=args.node_image,
    )

    try:
        if args.command == "create":
            if cluster_exists(cfg.cluster_name):
                print(f"Cluster '{cfg.cluster_name}' already exists")
                ensure_registry(cfg)
                kubectl("config", "use-context", f"kind-{cfg.cluster_name}")
                return 0

            ensure_registry(cfg)
            create_cluster(cfg)
            return 0

        elif args.command == "delete":
            delete_cluster(cfg)
            return 0

        elif args.command == "delete-all":
            delete_cluster(cfg)
            delete_registry(cfg)
            return 0

        elif args.command == "status":
            status(cfg)
            return 0

    except subprocess.CalledProcessError as e:
        print(f"Command failed: {e}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("\nInterrupted", file=sys.stderr)
        return 130

    return 0


if __name__ == "__main__":
    sys.exit(main())
