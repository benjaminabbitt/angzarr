"""Deterministic UUID generation for tests.

Provides functions for generating consistent, reproducible UUIDs
based on string names. This ensures tests produce the same IDs
across runs.
"""

from uuid import UUID, uuid5

# Default namespace for test UUIDs
# Can be overridden per-test-suite if needed
DEFAULT_TEST_NAMESPACE = UUID("a1b2c3d4-e5f6-7890-abcd-ef1234567890")


def uuid_for(name: str, namespace: UUID = DEFAULT_TEST_NAMESPACE) -> bytes:
    """Generate a deterministic 16-byte UUID from a name.

    The same name always generates the same UUID within a namespace.
    Returns bytes suitable for use as aggregate root IDs.

    Args:
        name: A string identifier (e.g., "player-alice", "table-1")
        namespace: UUID namespace for generation (defaults to test namespace)

    Returns:
        16-byte UUID as bytes

    Example:
        root = uuid_for("player-alice")
        assert len(root) == 16
        assert uuid_for("player-alice") == root  # deterministic
    """
    return uuid5(namespace, name).bytes


def uuid_str_for(name: str, namespace: UUID = DEFAULT_TEST_NAMESPACE) -> str:
    """Generate a deterministic UUID string from a name.

    Args:
        name: A string identifier
        namespace: UUID namespace for generation

    Returns:
        UUID as standard string format (8-4-4-4-12)

    Example:
        id_str = uuid_str_for("player-alice")
        assert "-" in id_str  # standard UUID format
    """
    return str(uuid5(namespace, name))


def uuid_obj_for(name: str, namespace: UUID = DEFAULT_TEST_NAMESPACE) -> UUID:
    """Generate a deterministic UUID object from a name.

    Args:
        name: A string identifier
        namespace: UUID namespace for generation

    Returns:
        UUID object

    Example:
        id_obj = uuid_obj_for("player-alice")
        assert id_obj.bytes == uuid_for("player-alice")
    """
    return uuid5(namespace, name)


__all__ = [
    "DEFAULT_TEST_NAMESPACE",
    "uuid_for",
    "uuid_str_for",
    "uuid_obj_for",
]
