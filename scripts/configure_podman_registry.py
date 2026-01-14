#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Configure Podman to trust the local Kind registry.

Adds an insecure registry entry to ~/.config/containers/registries.conf
for localhost:5001 (or specified port).
"""

import argparse
import os
import re
import sys
from pathlib import Path


def get_registries_conf_path() -> Path:
    """Get the path to registries.conf."""
    # User config takes precedence
    user_config = Path.home() / ".config" / "containers" / "registries.conf"
    return user_config


def registry_entry(port: int) -> str:
    """Generate the registry entry block."""
    return f'''[[registry]]
location="localhost:{port}"
insecure=true
'''


def check_registry_configured(content: str, port: int) -> bool:
    """Check if the registry is already configured."""
    pattern = rf'\[\[registry\]\]\s*\n\s*location\s*=\s*["\']?localhost:{port}["\']?'
    return bool(re.search(pattern, content, re.IGNORECASE))


def configure_registry(port: int, dry_run: bool = False) -> bool:
    """Configure the registry in registries.conf.

    Returns True if changes were made, False if already configured.
    """
    conf_path = get_registries_conf_path()

    # Read existing content
    existing_content = ""
    if conf_path.exists():
        existing_content = conf_path.read_text()

    # Check if already configured
    if check_registry_configured(existing_content, port):
        print(f"Registry localhost:{port} already configured in {conf_path}")
        return False

    # Prepare new content
    entry = registry_entry(port)
    if existing_content:
        new_content = existing_content.rstrip() + "\n\n" + entry
    else:
        new_content = entry

    if dry_run:
        print(f"Would write to {conf_path}:")
        print(entry)
        return True

    # Create directory if needed
    conf_path.parent.mkdir(parents=True, exist_ok=True)

    # Write config
    conf_path.write_text(new_content)
    print(f"Added localhost:{port} to {conf_path}")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Configure Podman to trust the local Kind registry"
    )
    parser.add_argument(
        "--port",
        type=int,
        default=int(os.environ.get("REGISTRY_PORT", "5001")),
        help="Registry port (default: 5001)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be done without making changes",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Check if registry is configured (exit 0 if yes, 1 if no)",
    )

    args = parser.parse_args()

    if args.check:
        conf_path = get_registries_conf_path()
        if conf_path.exists():
            content = conf_path.read_text()
            if check_registry_configured(content, args.port):
                print(f"Registry localhost:{args.port} is configured")
                return 0
        print(f"Registry localhost:{args.port} is NOT configured")
        return 1

    try:
        configure_registry(args.port, args.dry_run)
        return 0
    except PermissionError as e:
        print(f"Permission denied: {e}", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
