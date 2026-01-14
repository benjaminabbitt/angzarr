#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# ///
"""Secrets management for angzarr Kubernetes deployment.

Generates secure credentials and stores them in Kubernetes secrets.
"""

import argparse
import base64
import json
import os
import secrets
import subprocess
import sys
from dataclasses import dataclass


@dataclass
class Config:
    namespace: str = "angzarr"
    secrets_namespace: str = "angzarr-secrets"
    secret_name: str = "angzarr-secrets"


def generate_password(length: int = 32) -> str:
    """Generate a cryptographically secure alphanumeric password."""
    alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
    return "".join(secrets.choice(alphabet) for _ in range(length))


def generate_erlang_cookie() -> str:
    """Generate a secure Erlang cookie for RabbitMQ clustering."""
    return base64.b64encode(secrets.token_bytes(32)).decode("ascii")


def kubectl(*args: str, capture: bool = False) -> subprocess.CompletedProcess:
    """Run kubectl command."""
    cmd = ["kubectl", *args]
    if capture:
        return subprocess.run(cmd, capture_output=True, text=True, check=False)
    return subprocess.run(cmd, check=False)


def namespace_exists(namespace: str) -> bool:
    """Check if a Kubernetes namespace exists."""
    result = kubectl("get", "namespace", namespace, capture=True)
    return result.returncode == 0


def secret_exists(name: str, namespace: str) -> bool:
    """Check if a Kubernetes secret exists."""
    result = kubectl("get", "secret", name, "-n", namespace, capture=True)
    return result.returncode == 0


def create_namespace(namespace: str) -> None:
    """Create namespace if it doesn't exist."""
    if not namespace_exists(namespace):
        print(f"Creating namespace: {namespace}")
        kubectl("create", "namespace", namespace)
    else:
        print(f"Namespace already exists: {namespace}")


def get_secret_data(name: str, namespace: str) -> dict[str, str] | None:
    """Get secret data from Kubernetes (base64 decoded)."""
    result = kubectl(
        "get", "secret", name, "-n", namespace, "-o", "json", capture=True
    )
    if result.returncode != 0:
        return None

    secret = json.loads(result.stdout)
    data = secret.get("data", {})
    return {k: base64.b64decode(v).decode("utf-8") for k, v in data.items()}


def create_secret(
    name: str,
    namespace: str,
    data: dict[str, str],
    force: bool = False,
) -> None:
    """Create or update a Kubernetes secret."""
    if secret_exists(name, namespace) and not force:
        print(f"Secret '{name}' already exists in namespace '{namespace}'")
        print("Use --force to overwrite")
        return

    # Build kubectl command with literals
    cmd = [
        "kubectl",
        "create",
        "secret",
        "generic",
        name,
        "--namespace",
        namespace,
        "--dry-run=client",
        "-o",
        "yaml",
    ]
    for key, value in data.items():
        cmd.append(f"--from-literal={key}={value}")

    # Generate YAML and apply
    result = subprocess.run(cmd, capture_output=True, text=True, check=True)
    apply_result = subprocess.run(
        ["kubectl", "apply", "-f", "-"],
        input=result.stdout,
        capture_output=True,
        text=True,
        check=True,
    )
    print(apply_result.stdout.strip())


def generate_credentials(env_override: bool = True) -> dict[str, str]:
    """Generate all required credentials.

    Args:
        env_override: If True, use environment variables when set.

    Returns:
        Dictionary of credential names to values.
    """
    credentials = {}

    # MongoDB root password
    if env_override and os.environ.get("MONGODB_ROOT_PASSWORD"):
        credentials["mongodb-root-password"] = os.environ["MONGODB_ROOT_PASSWORD"]
        print("Using MONGODB_ROOT_PASSWORD from environment")
    else:
        credentials["mongodb-root-password"] = generate_password()
        print("Generated MongoDB root password")

    # MongoDB user password
    if env_override and os.environ.get("MONGODB_PASSWORD"):
        credentials["mongodb-password"] = os.environ["MONGODB_PASSWORD"]
        print("Using MONGODB_PASSWORD from environment")
    else:
        credentials["mongodb-password"] = generate_password()
        print("Generated MongoDB user password")

    # RabbitMQ password
    if env_override and os.environ.get("RABBITMQ_PASSWORD"):
        credentials["rabbitmq-password"] = os.environ["RABBITMQ_PASSWORD"]
        print("Using RABBITMQ_PASSWORD from environment")
    else:
        credentials["rabbitmq-password"] = generate_password()
        print("Generated RabbitMQ password")

    # RabbitMQ Erlang cookie
    if env_override and os.environ.get("RABBITMQ_ERLANG_COOKIE"):
        credentials["rabbitmq-erlang-cookie"] = os.environ["RABBITMQ_ERLANG_COOKIE"]
        print("Using RABBITMQ_ERLANG_COOKIE from environment")
    else:
        credentials["rabbitmq-erlang-cookie"] = generate_erlang_cookie()
        print("Generated RabbitMQ Erlang cookie")

    return credentials


def cmd_init(args: argparse.Namespace, config: Config) -> int:
    """Initialize secrets (idempotent - won't overwrite existing)."""
    create_namespace(config.secrets_namespace)

    if secret_exists(config.secret_name, config.secrets_namespace) and not args.force:
        print(f"Secrets already exist in namespace '{config.secrets_namespace}'")
        print("Use --force to regenerate")
        return 0

    credentials = generate_credentials()
    create_secret(
        config.secret_name,
        config.secrets_namespace,
        credentials,
        force=args.force,
    )

    print()
    print("Credentials securely generated and stored in Kubernetes.")
    print("They are NOT saved anywhere else. To view them:")
    print(f"  just secrets-show")
    return 0


def cmd_rotate(args: argparse.Namespace, config: Config) -> int:
    """Rotate all secrets (force regenerate)."""
    args.force = True
    return cmd_init(args, config)


def cmd_show(args: argparse.Namespace, config: Config) -> int:
    """Display current secrets."""
    if not secret_exists(config.secret_name, config.secrets_namespace):
        print(f"No secrets found in namespace '{config.secrets_namespace}'")
        return 1

    data = get_secret_data(config.secret_name, config.secrets_namespace)
    if not data:
        print("Failed to retrieve secret data")
        return 1

    print(f"Secrets in namespace '{config.secrets_namespace}':")
    for key, value in sorted(data.items()):
        # Mask most of the value for security
        if len(value) > 8:
            masked = value[:4] + "*" * (len(value) - 8) + value[-4:]
        else:
            masked = "*" * len(value)

        if args.reveal:
            print(f"  {key}: {value}")
        else:
            print(f"  {key}: {masked}")

    if not args.reveal:
        print()
        print("Values are masked. Use --reveal to show full values.")

    return 0


def cmd_check(args: argparse.Namespace, config: Config) -> int:
    """Check if secrets exist."""
    if secret_exists(config.secret_name, config.secrets_namespace):
        print(f"Secrets exist in namespace '{config.secrets_namespace}'")
        return 0
    else:
        print(f"No secrets found in namespace '{config.secrets_namespace}'")
        return 1


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Manage secrets for angzarr Kubernetes deployment"
    )
    parser.add_argument(
        "--namespace",
        default=os.environ.get("NAMESPACE", "angzarr"),
        help="Target namespace (default: angzarr)",
    )
    parser.add_argument(
        "--secrets-namespace",
        default=os.environ.get("SECRETS_NAMESPACE", "angzarr-secrets"),
        help="Secrets source namespace (default: angzarr-secrets)",
    )

    subparsers = parser.add_subparsers(dest="command", required=True)

    # init command
    init_parser = subparsers.add_parser(
        "init", help="Initialize secrets (idempotent)"
    )
    init_parser.add_argument(
        "--force", action="store_true", help="Overwrite existing secrets"
    )

    # rotate command
    subparsers.add_parser("rotate", help="Rotate all secrets")

    # show command
    show_parser = subparsers.add_parser("show", help="Display current secrets")
    show_parser.add_argument(
        "--reveal", action="store_true", help="Show full secret values"
    )

    # check command
    subparsers.add_parser("check", help="Check if secrets exist")

    args = parser.parse_args()

    config = Config(
        namespace=args.namespace,
        secrets_namespace=args.secrets_namespace,
    )

    commands = {
        "init": cmd_init,
        "rotate": cmd_rotate,
        "show": cmd_show,
        "check": cmd_check,
    }

    return commands[args.command](args, config)


if __name__ == "__main__":
    sys.exit(main())
