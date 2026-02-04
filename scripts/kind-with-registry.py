#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Create a kind cluster with a local container registry.

Creates a kind cluster configured to use a local registry for faster
image pulls. Uses podman as the container runtime.

Based on: https://github.com/bkuzmic/skaffold-podman-kind

Cluster configuration (ports, mounts, containerd patches) lives in
kind-config.yaml at the repo root. This script handles lifecycle
orchestration only.
"""

import argparse
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent


@dataclass
class Config:
    """Cluster configuration."""

    cluster_name: str = "angzarr"
    registry_name: str = "kind-registry"
    registry_port: int = 5001
    # Use k8s 1.31 for better rootless podman compatibility (1.35 has issues)
    node_image: str = "kindest/node:v1.31.4"


def run(
    *args: str,
    capture: bool = False,
    check: bool = True,
    env: dict[str, str] | None = None,
    cwd: Path | None = None,
) -> subprocess.CompletedProcess:
    """Run a command."""
    full_env = {**os.environ, **(env or {})}
    if capture:
        return subprocess.run(
            args, capture_output=True, text=True, check=check, env=full_env, cwd=cwd
        )
    return subprocess.run(args, check=check, env=full_env, cwd=cwd)


def podman(*args: str, capture: bool = False, check: bool = True) -> subprocess.CompletedProcess:
    """Run podman command."""
    return run("podman", *args, capture=capture, check=check)


def kind(
    *args: str,
    capture: bool = False,
    check: bool = True,
    use_scope: bool = False,
    cwd: Path | None = None,
) -> subprocess.CompletedProcess:
    """Run kind command with podman provider.

    Args:
        use_scope: Wrap with systemd-run --user --scope for rootless cgroup delegation.
        cwd: Working directory for the command.
    """
    env = {"KIND_EXPERIMENTAL_PROVIDER": "podman"}
    if use_scope and os.getuid() != 0:
        return run("systemd-run", "--user", "--scope", "kind", *args, capture=capture, check=check, env=env, cwd=cwd)
    return run("kind", *args, capture=capture, check=check, env=env, cwd=cwd)


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


def network_exists(name: str) -> bool:
    """Check if a podman network exists."""
    result = podman("network", "inspect", name, capture=True, check=False)
    return result.returncode == 0


def registry_on_network(registry_name: str, network_name: str) -> bool:
    """Check if registry is connected to the specified network."""
    result = podman(
        "container", "inspect", registry_name,
        "--format", f"{{{{index .NetworkSettings.Networks \"{network_name}\"}}}}",
        capture=True, check=False
    )
    # If network exists, output is non-empty (the network config map)
    return result.returncode == 0 and result.stdout.strip() not in ("", "<no value>")


def ensure_volume(name: str) -> None:
    """Ensure a podman volume exists."""
    result = podman("volume", "inspect", name, capture=True, check=False)
    if result.returncode != 0:
        print(f"Creating persistent volume '{name}'...")
        podman("volume", "create", name)
    else:
        print(f"Volume '{name}' already exists")


def ensure_registry(cfg: Config, require_kind_network: bool = False) -> None:
    """Ensure the registry container is running with persistent storage.

    Args:
        cfg: Cluster configuration.
        require_kind_network: If True, requires kind network to exist and creates
            registry on it. If False, creates registry without network requirement.
    """
    volume_name = f"{cfg.registry_name}-data"
    ensure_volume(volume_name)

    kind_network_available = network_exists("kind")

    if registry_exists(cfg.registry_name):
        # Registry exists - check if it needs to be recreated on kind network
        if kind_network_available and not registry_on_network(cfg.registry_name, "kind"):
            print("Registry exists but not on kind network, recreating...")
            podman("stop", cfg.registry_name, check=False)
            podman("rm", cfg.registry_name, check=False)
        elif not registry_running(cfg.registry_name):
            print("Starting stopped registry...")
            podman("start", cfg.registry_name)
            return
        else:
            print("Registry already running on correct network")
            return

    # Create registry - use kind network if available
    if kind_network_available:
        print(f"Creating local registry on port {cfg.registry_port} (network: kind)...")
        podman(
            "run", "-d",
            "--restart=always",
            "--network", "kind",
            "-p", f"127.0.0.1:{cfg.registry_port}:5000",
            "-v", f"{volume_name}:/var/lib/registry",
            "-e", "REGISTRY_STORAGE_DELETE_ENABLED=true",
            "--name", cfg.registry_name,
            "docker.io/library/registry:2",
        )
    elif require_kind_network:
        print("Error: kind network required but does not exist", file=sys.stderr)
        sys.exit(1)
    else:
        print(f"Creating local registry on port {cfg.registry_port} (no kind network yet)...")
        podman(
            "run", "-d",
            "--restart=always",
            "-p", f"127.0.0.1:{cfg.registry_port}:5000",
            "-v", f"{volume_name}:/var/lib/registry",
            "-e", "REGISTRY_STORAGE_DELETE_ENABLED=true",
            "--name", cfg.registry_name,
            "docker.io/library/registry:2",
        )


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
    """Create the kind cluster using kind-config.yaml from the repo root.

    The config file uses extraMounts with a relative hostPath
    (.kind-registry-config), so we run kind from the repo root to
    resolve it correctly.
    """
    config_path = REPO_ROOT / "kind-config.yaml"
    if not config_path.exists():
        print(f"Error: {config_path} not found", file=sys.stderr)
        sys.exit(1)

    print(f"Creating kind cluster '{cfg.cluster_name}' with {cfg.node_image}...")
    # Run from repo root so extraMounts relative hostPath resolves correctly
    kind("create", "cluster", "--name", cfg.cluster_name,
         "--config", str(config_path), "--image", cfg.node_image,
         use_scope=True, cwd=REPO_ROOT)

    # Now that kind network exists, ensure registry is on it
    ensure_registry(cfg, require_kind_network=True)

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
        on_kind = registry_on_network(cfg.registry_name, "kind")
        state = "RUNNING" if running else "STOPPED"
        network_state = "kind" if on_kind else "NOT on kind network"
        print(f"Registry '{cfg.registry_name}': {state} ({network_state})")
        if running:
            # Test registry connectivity
            import urllib.request
            try:
                with urllib.request.urlopen(f"http://localhost:{cfg.registry_port}/v2/", timeout=2) as resp:
                    if resp.status == 200:
                        print(f"Registry API: OK (http://localhost:{cfg.registry_port}/v2/)")
            except Exception as e:
                print(f"Registry API: UNREACHABLE ({e})")
            if not on_kind and cluster_exists(cfg.cluster_name):
                print("WARNING: Registry not on kind network - pods cannot pull images!")
                print("Run 'just cluster-create' to fix this.")
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
                # Cluster exists, so kind network exists - ensure registry is on it
                ensure_registry(cfg, require_kind_network=True)
                kubectl("config", "use-context", f"kind-{cfg.cluster_name}")
                return 0

            # Create cluster first (establishes kind network), then registry
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
