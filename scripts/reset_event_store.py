#!/usr/bin/env python3
"""Reset the event store and message queues before acceptance tests.

Truncates the angzarr PostgreSQL tables and purges RabbitMQ queues so tests
start with a clean slate. Uses kubectl to read credentials from k8s
secrets and execute commands inside pods.

Usage:
    reset_event_store.py [--namespace NAMESPACE] [--database DATABASE]
"""

import argparse
import base64
import subprocess
import sys


def get_secret_value(namespace: str, secret_name: str, key: str) -> str:
    """Read a value from a k8s secret."""
    result = subprocess.run(
        [
            "kubectl",
            "get",
            "secret",
            "-n",
            namespace,
            secret_name,
            "-o",
            f"jsonpath={{.data.{key}}}",
        ],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Failed to read secret {secret_name}/{key}: {result.stderr}")
    return base64.b64decode(result.stdout).decode()


def get_pod(namespace: str, label: str) -> str:
    """Find a pod by label selector."""
    result = subprocess.run(
        [
            "kubectl",
            "get",
            "pods",
            "-n",
            namespace,
            "-l",
            label,
            "-o",
            "jsonpath={.items[0].metadata.name}",
        ],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout:
        raise RuntimeError(f"No pod found with label {label} in namespace {namespace}")
    return result.stdout


def truncate_database(namespace: str, database: str) -> bool:
    """Truncate PostgreSQL tables via kubectl exec."""
    try:
        password = get_secret_value(namespace, f"{namespace}-db-postgresql", "password")
    except RuntimeError:
        print(f"  No PostgreSQL secret found in {namespace}, skipping", file=sys.stderr)
        return True

    try:
        pod = get_pod(namespace, "app.kubernetes.io/name=postgresql")
    except RuntimeError:
        print(f"  No PostgreSQL pod found in {namespace}, skipping", file=sys.stderr)
        return True

    # Truncate all angzarr tables
    truncate_sql = """
    DO $$
    DECLARE
        tbl TEXT;
    BEGIN
        FOR tbl IN
            SELECT tablename FROM pg_tables
            WHERE schemaname = 'public'
        LOOP
            EXECUTE 'TRUNCATE TABLE ' || quote_ident(tbl) || ' CASCADE';
        END LOOP;
    END $$;
    """

    result = subprocess.run(
        [
            "kubectl",
            "exec",
            "-n",
            namespace,
            pod,
            "--",
            "psql",
            "-U", "angzarr",
            "-d", database,
            "-c", truncate_sql,
        ],
        capture_output=True,
        text=True,
        env={"PGPASSWORD": password, **dict(__import__("os").environ)},
    )

    if result.returncode != 0:
        print(f"  Failed to truncate database: {result.stderr}", file=sys.stderr)
        return False

    return True


def purge_rabbitmq_queues(namespace: str) -> bool:
    """Purge all angzarr RabbitMQ queues via kubectl exec + rabbitmqctl."""
    try:
        pod = get_pod(namespace, "app.kubernetes.io/name=rabbitmq")
    except RuntimeError:
        print(f"  No RabbitMQ pod found in {namespace}, skipping", file=sys.stderr)
        return True

    # List queues
    result = subprocess.run(
        [
            "kubectl",
            "exec",
            "-n",
            namespace,
            pod,
            "-c",
            "rabbitmq",
            "--",
            "rabbitmqctl",
            "list_queues",
            "name",
            "--quiet",
        ],
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print(f"  Failed to list queues: {result.stderr}", file=sys.stderr)
        return False

    # Filter out column headers and blank lines
    column_names = {"name", "messages", "consumers", "state", "type"}
    queues = [
        q.strip()
        for q in result.stdout.strip().splitlines()
        if q.strip() and q.strip().lower() not in column_names
    ]
    if not queues:
        print("  No queues to purge")
        return True

    ok = True
    for queue in queues:
        result = subprocess.run(
            [
                "kubectl",
                "exec",
                "-n",
                namespace,
                pod,
                "-c",
                "rabbitmq",
                "--",
                "rabbitmqctl",
                "purge_queue",
                queue,
            ],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(f"  Failed to purge queue {queue}: {result.stderr}", file=sys.stderr)
            ok = False
        else:
            print(f"  Purged queue: {queue}")

    return ok


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Reset the event store and message queues before acceptance tests."
    )
    parser.add_argument(
        "--namespace",
        default="angzarr",
        help="Kubernetes namespace (default: angzarr)",
    )
    parser.add_argument(
        "--database",
        default="angzarr",
        help="PostgreSQL database name (default: angzarr)",
    )
    args = parser.parse_args()

    print(f"Resetting event store ({args.database})...")

    if not truncate_database(args.namespace, args.database):
        return 1

    print("  Event store reset complete")

    print("Purging message queues...")

    if not purge_rabbitmq_queues(args.namespace):
        return 1

    print("  Message queues purged")
    return 0


if __name__ == "__main__":
    sys.exit(main())
