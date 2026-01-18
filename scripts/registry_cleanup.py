#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = ["httpx"]
# ///
"""Registry image cleanup for angzarr development.

Cleans up old images from the local Kind registry based on age or pattern.
Supports TTL-based expiry, pattern matching, and dry-run mode.

Registry API v2 endpoints used:
  GET /v2/_catalog - list repositories
  GET /v2/{repo}/tags/list - list tags
  GET /v2/{repo}/manifests/{ref} - get manifest (includes digest header)
  DELETE /v2/{repo}/manifests/{digest} - delete manifest
"""

import argparse
import os
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone


@dataclass
class Config:
    """Cleanup configuration."""

    registry_url: str = "http://localhost:5001"
    registry_name: str = "kind-registry"
    max_age_hours: int = 24
    keep_tags: list[str] = field(default_factory=lambda: ["latest"])
    dry_run: bool = False


def get_client():
    """Get httpx client with appropriate headers."""
    import httpx

    return httpx.Client(
        timeout=30.0,
        headers={
            "Accept": "application/vnd.docker.distribution.manifest.v2+json",
        },
    )


def list_repositories(cfg: Config) -> list[str]:
    """List all repositories in the registry."""
    client = get_client()
    try:
        resp = client.get(f"{cfg.registry_url}/v2/_catalog")
        resp.raise_for_status()
        data = resp.json()
        return data.get("repositories", [])
    except Exception as e:
        print(f"Error listing repositories: {e}", file=sys.stderr)
        return []
    finally:
        client.close()


def list_tags(cfg: Config, repo: str) -> list[str]:
    """List all tags for a repository."""
    client = get_client()
    try:
        resp = client.get(f"{cfg.registry_url}/v2/{repo}/tags/list")
        if resp.status_code == 404:
            return []
        resp.raise_for_status()
        data = resp.json()
        return data.get("tags", []) or []
    except Exception as e:
        print(f"Error listing tags for {repo}: {e}", file=sys.stderr)
        return []
    finally:
        client.close()


def get_manifest_digest(cfg: Config, repo: str, tag: str) -> str | None:
    """Get the digest for a manifest by tag."""
    client = get_client()
    try:
        resp = client.head(f"{cfg.registry_url}/v2/{repo}/manifests/{tag}")
        if resp.status_code == 404:
            return None
        resp.raise_for_status()
        return resp.headers.get("Docker-Content-Digest")
    except Exception as e:
        print(f"Error getting digest for {repo}:{tag}: {e}", file=sys.stderr)
        return None
    finally:
        client.close()


def delete_manifest(cfg: Config, repo: str, digest: str) -> bool:
    """Delete a manifest by digest."""
    if cfg.dry_run:
        print(f"  [DRY RUN] Would delete {repo}@{digest}")
        return True

    client = get_client()
    try:
        resp = client.delete(f"{cfg.registry_url}/v2/{repo}/manifests/{digest}")
        if resp.status_code == 202:
            print(f"  Deleted {repo}@{digest[:19]}...")
            return True
        elif resp.status_code == 405:
            print(f"  Error: Registry delete not enabled (405)", file=sys.stderr)
            return False
        else:
            print(f"  Error deleting {repo}@{digest}: {resp.status_code}", file=sys.stderr)
            return False
    except Exception as e:
        print(f"  Error deleting {repo}@{digest}: {e}", file=sys.stderr)
        return False
    finally:
        client.close()


def run_garbage_collect(cfg: Config) -> bool:
    """Run registry garbage collection via podman exec."""
    if cfg.dry_run:
        print("[DRY RUN] Would run garbage collection")
        return True

    print("Running garbage collection...")
    try:
        result = subprocess.run(
            [
                "podman", "exec", cfg.registry_name,
                "registry", "garbage-collect",
                "/etc/docker/registry/config.yml",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode == 0:
            print("Garbage collection complete")
            return True
        else:
            print(f"Garbage collection failed: {result.stderr}", file=sys.stderr)
            return False
    except FileNotFoundError:
        print("Error: podman not found", file=sys.stderr)
        return False


def is_sha256_tag(tag: str) -> bool:
    """Check if a tag looks like a sha256 content hash."""
    if len(tag) == 64:
        try:
            int(tag, 16)
            return True
        except ValueError:
            pass
    return False


def cmd_status(cfg: Config) -> int:
    """Show registry status."""
    print(f"Registry: {cfg.registry_url}")
    print()

    repos = list_repositories(cfg)
    if not repos:
        print("No repositories found (or registry unreachable)")
        return 1

    total_tags = 0
    sha256_tags = 0

    print(f"{'Repository':<40} {'Tags':>6} {'SHA256':>8}")
    print("-" * 56)

    for repo in sorted(repos):
        tags = list_tags(cfg, repo)
        tag_count = len(tags)
        sha_count = sum(1 for t in tags if is_sha256_tag(t))
        total_tags += tag_count
        sha256_tags += sha_count
        print(f"{repo:<40} {tag_count:>6} {sha_count:>8}")

    print("-" * 56)
    print(f"{'Total':<40} {total_tags:>6} {sha256_tags:>8}")
    return 0


def cmd_list(cfg: Config) -> int:
    """List all images."""
    repos = list_repositories(cfg)
    if not repos:
        print("No repositories found")
        return 1

    for repo in sorted(repos):
        tags = list_tags(cfg, repo)
        if not tags:
            continue
        print(f"\n{repo}:")
        for tag in sorted(tags):
            marker = " [sha256]" if is_sha256_tag(tag) else ""
            keep = " [keep]" if tag in cfg.keep_tags else ""
            print(f"  {tag}{marker}{keep}")

    return 0


def cmd_clean_sha256(cfg: Config) -> int:
    """Delete all sha256-tagged images (keep latest and other named tags)."""
    repos = list_repositories(cfg)
    if not repos:
        print("No repositories found")
        return 0

    deleted = 0
    failed = 0

    for repo in sorted(repos):
        tags = list_tags(cfg, repo)
        sha256_tags = [t for t in tags if is_sha256_tag(t)]

        if not sha256_tags:
            continue

        print(f"\n{repo}: {len(sha256_tags)} sha256 tags")
        for tag in sha256_tags:
            digest = get_manifest_digest(cfg, repo, tag)
            if digest:
                if delete_manifest(cfg, repo, digest):
                    deleted += 1
                else:
                    failed += 1

    print(f"\nDeleted: {deleted}, Failed: {failed}")
    return 0 if failed == 0 else 1


def cmd_clean_repo(cfg: Config, repo: str) -> int:
    """Delete all tags for a specific repository."""
    tags = list_tags(cfg, repo)
    if not tags:
        print(f"No tags found for {repo}")
        return 0

    deleted = 0
    failed = 0

    print(f"{repo}: {len(tags)} tags")
    for tag in tags:
        digest = get_manifest_digest(cfg, repo, tag)
        if digest:
            if delete_manifest(cfg, repo, digest):
                deleted += 1
            else:
                failed += 1

    print(f"\nDeleted: {deleted}, Failed: {failed}")
    return 0 if failed == 0 else 1


def cmd_clean_all(cfg: Config) -> int:
    """Delete ALL images from the registry."""
    repos = list_repositories(cfg)
    if not repos:
        print("No repositories found")
        return 0

    deleted = 0
    failed = 0

    for repo in sorted(repos):
        tags = list_tags(cfg, repo)
        if not tags:
            continue

        print(f"\n{repo}: {len(tags)} tags")
        for tag in tags:
            digest = get_manifest_digest(cfg, repo, tag)
            if digest:
                if delete_manifest(cfg, repo, digest):
                    deleted += 1
                else:
                    failed += 1

    print(f"\nDeleted: {deleted}, Failed: {failed}")
    return 0 if failed == 0 else 1


def cmd_gc(cfg: Config) -> int:
    """Run garbage collection only."""
    return 0 if run_garbage_collect(cfg) else 1


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Registry image cleanup for angzarr development",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Commands:
  status       Show registry status (repository and tag counts)
  list         List all images in the registry
  clean-sha256 Delete all sha256-tagged images (keep named tags)
  clean-repo   Delete all tags for a specific repository
  clean-all    Delete ALL images (full reset)
  gc           Run garbage collection (reclaim disk after deletes)

Examples:
  %(prog)s status
  %(prog)s clean-sha256 --dry-run
  %(prog)s clean-repo angzarr-aggregate
  %(prog)s gc
        """,
    )

    parser.add_argument(
        "command",
        choices=["status", "list", "clean-sha256", "clean-repo", "clean-all", "gc"],
        help="Command to run",
    )
    parser.add_argument(
        "repo",
        nargs="?",
        help="Repository name (for clean-repo command)",
    )
    parser.add_argument(
        "--registry-url",
        default=os.environ.get("REGISTRY_URL", "http://localhost:5001"),
        help="Registry URL (default: http://localhost:5001)",
    )
    parser.add_argument(
        "--registry-name",
        default=os.environ.get("REGISTRY_NAME", "kind-registry"),
        help="Registry container name for GC (default: kind-registry)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be deleted without actually deleting",
    )

    args = parser.parse_args()

    cfg = Config(
        registry_url=args.registry_url,
        registry_name=args.registry_name,
        dry_run=args.dry_run,
    )

    if args.command == "status":
        return cmd_status(cfg)
    elif args.command == "list":
        return cmd_list(cfg)
    elif args.command == "clean-sha256":
        return cmd_clean_sha256(cfg)
    elif args.command == "clean-repo":
        if not args.repo:
            print("Error: clean-repo requires a repository name", file=sys.stderr)
            return 1
        return cmd_clean_repo(cfg, args.repo)
    elif args.command == "clean-all":
        return cmd_clean_all(cfg)
    elif args.command == "gc":
        return cmd_gc(cfg)

    return 0


if __name__ == "__main__":
    sys.exit(main())
