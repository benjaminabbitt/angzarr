#!/usr/bin/env python3
"""Wait for gRPC services to become healthy using grpc.health.v1.Health/Check.

Usage:
    wait-for-grpc-health.sh [--timeout SECONDS] [--interval SECONDS] HOST:PORT...

Example:
    wait-for-grpc-health.sh localhost:50051 localhost:50052
    wait-for-grpc-health.sh --timeout 120 --interval 2 localhost:50051
"""

import argparse
import os
import subprocess
import sys
import time

# Find the proto directory relative to this script
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROTO_DIR = os.path.join(SCRIPT_DIR, "..", "proto")


def check_health(endpoint: str) -> bool:
    """Check gRPC health endpoint, returns True if SERVING."""
    try:
        result = subprocess.run(
            [
                "grpcurl",
                "-plaintext",
                "-connect-timeout",
                "5",
                "-import-path",
                PROTO_DIR,
                "-proto",
                "health/v1/health.proto",
                "-d",
                '{"service": ""}',
                endpoint,
                "grpc.health.v1.Health/Check",
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )
        return '"status": "SERVING"' in result.stdout or '"status":"SERVING"' in result.stdout
    except (subprocess.TimeoutExpired, subprocess.SubprocessError):
        return False


def check_grpcurl_installed() -> bool:
    """Check if grpcurl is available."""
    try:
        subprocess.run(["grpcurl", "--version"], capture_output=True, check=True)
        return True
    except (subprocess.SubprocessError, FileNotFoundError):
        return False


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Wait for gRPC services to respond SERVING to health checks."
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=180,
        help="Maximum wait time in seconds (default: 180)",
    )
    parser.add_argument(
        "--interval",
        type=int,
        default=5,
        help="Poll interval in seconds (default: 5)",
    )
    parser.add_argument(
        "services",
        nargs="+",
        metavar="HOST:PORT",
        help="gRPC service endpoints to check",
    )

    args = parser.parse_args()

    if not check_grpcurl_installed():
        print("Error: grpcurl is not installed", file=sys.stderr)
        print(
            "Install with: brew install grpcurl, go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest",
            file=sys.stderr,
        )
        return 1

    print("Waiting for gRPC services to become healthy...")
    print(f"  Timeout: {args.timeout}s, Interval: {args.interval}s")
    print(f"  Services: {' '.join(args.services)}")
    print()

    start_time = time.time()
    healthy_services: set[str] = set()

    while True:
        elapsed = time.time() - start_time

        if elapsed >= args.timeout:
            print()
            print("Timeout waiting for services. Unhealthy services:")
            for service in args.services:
                if service not in healthy_services:
                    print(f"  - {service}")
            return 1

        all_healthy = True
        for service in args.services:
            if service not in healthy_services:
                if check_health(service):
                    healthy_services.add(service)
                    print(f"  [OK] {service} is healthy")
                else:
                    all_healthy = False

        if all_healthy:
            print()
            print("All services healthy!")
            return 0

        remaining = int(args.timeout - elapsed)
        print(f"  Waiting... ({remaining}s remaining)")
        time.sleep(args.interval)


if __name__ == "__main__":
    sys.exit(main())
