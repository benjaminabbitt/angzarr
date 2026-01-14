#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["pyyaml"]
# ///
"""Configure Skaffold for Kind with local registry.

Sets kind-disable-load: true in ~/.skaffold/config to use registry
push instead of kind load (required for Podman compatibility).
"""

import argparse
import os
import sys
from pathlib import Path

import yaml


def get_skaffold_config_path() -> Path:
    """Get the path to Skaffold config."""
    return Path.home() / ".skaffold" / "config"


def check_kind_disable_load(config: dict) -> bool:
    """Check if kind-disable-load is already set."""
    global_config = config.get("global", {})
    return global_config.get("kind-disable-load", False) is True


def configure_skaffold(dry_run: bool = False) -> bool:
    """Configure Skaffold for Kind with local registry.

    Returns True if changes were made, False if already configured.
    """
    conf_path = get_skaffold_config_path()

    # Read existing config
    config = {}
    if conf_path.exists():
        content = conf_path.read_text()
        if content.strip():
            config = yaml.safe_load(content) or {}

    # Check if already configured
    if check_kind_disable_load(config):
        print(f"kind-disable-load already set in {conf_path}")
        return False

    # Update config
    if "global" not in config:
        config["global"] = {}
    config["global"]["kind-disable-load"] = True

    if dry_run:
        print(f"Would write to {conf_path}:")
        print(yaml.dump(config, default_flow_style=False))
        return True

    # Create directory if needed
    conf_path.parent.mkdir(parents=True, exist_ok=True)

    # Write config
    conf_path.write_text(yaml.dump(config, default_flow_style=False))
    print(f"Set kind-disable-load: true in {conf_path}")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Configure Skaffold for Kind with local registry"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be done without making changes",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Check if configured (exit 0 if yes, 1 if no)",
    )

    args = parser.parse_args()

    if args.check:
        conf_path = get_skaffold_config_path()
        if conf_path.exists():
            content = conf_path.read_text()
            if content.strip():
                config = yaml.safe_load(content) or {}
                if check_kind_disable_load(config):
                    print("Skaffold kind-disable-load is configured")
                    return 0
        print("Skaffold kind-disable-load is NOT configured")
        return 1

    try:
        configure_skaffold(args.dry_run)
        return 0
    except PermissionError as e:
        print(f"Permission denied: {e}", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
