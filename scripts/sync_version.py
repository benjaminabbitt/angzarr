#!/usr/bin/env python3
"""Sync VERSION file to Cargo.toml and Chart.yaml."""

import re
import sys
from pathlib import Path


def read_version(repo_root: Path) -> str:
    """Read version from VERSION file."""
    version_file = repo_root / "VERSION"
    if not version_file.exists():
        print(f"Error: {version_file} not found", file=sys.stderr)
        sys.exit(1)
    return version_file.read_text().strip()


def update_cargo_toml(repo_root: Path, version: str) -> bool:
    """Update version in Cargo.toml [package] section."""
    cargo_file = repo_root / "Cargo.toml"
    if not cargo_file.exists():
        print(f"Warning: {cargo_file} not found", file=sys.stderr)
        return False

    content = cargo_file.read_text()

    # Match version in [package] section (after [package] but before next section)
    # This pattern finds: version = "x.y.z" in the package section
    pattern = r'(\[package\].*?version\s*=\s*")[^"]+(")'
    replacement = rf'\g<1>{version}\g<2>'

    new_content, count = re.subn(pattern, replacement, content, count=1, flags=re.DOTALL)

    if count == 0:
        print("Warning: Could not find version in Cargo.toml [package] section", file=sys.stderr)
        return False

    cargo_file.write_text(new_content)
    print(f"Updated Cargo.toml to version {version}")
    return True


def update_chart_yaml(repo_root: Path, version: str) -> bool:
    """Update version and appVersion in Chart.yaml."""
    chart_file = repo_root / "deploy" / "helm" / "angzarr" / "Chart.yaml"
    if not chart_file.exists():
        print(f"Warning: {chart_file} not found", file=sys.stderr)
        return False

    content = chart_file.read_text()

    # Update version field
    content = re.sub(r'^version:\s*.+$', f'version: {version}', content, flags=re.MULTILINE)

    # Update appVersion field
    content = re.sub(r'^appVersion:\s*.+$', f'appVersion: "{version}"', content, flags=re.MULTILINE)

    chart_file.write_text(content)
    print(f"Updated Chart.yaml to version {version}")
    return True


def main() -> int:
    """Main entry point."""
    # Find repo root (where VERSION file lives)
    script_dir = Path(__file__).parent
    repo_root = script_dir.parent

    version = read_version(repo_root)
    print(f"Syncing version: {version}")

    success = True
    success &= update_cargo_toml(repo_root, version)
    success &= update_chart_yaml(repo_root, version)

    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())
