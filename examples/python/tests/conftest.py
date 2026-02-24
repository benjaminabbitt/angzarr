"""Shared pytest fixtures for poker tests."""

import sys
from pathlib import Path

import pytest

# Add paths for imports
root = Path(__file__).parent.parent
sys.path.insert(0, str(root))
sys.path.insert(0, str(root / "player" / "agg"))
sys.path.insert(0, str(root / "table" / "agg"))
sys.path.insert(0, str(root / "hand" / "agg"))

# Import testing utilities from angzarr_client.testing
# Import proto modules for fixtures
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import poker_types_pb2 as poker_types
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.testing import (
    ScenarioContext,
    make_command_book,
    make_cover,
    make_event_book,
    make_timestamp,
    pack_event,
    uuid_for,
    uuid_str_for,
)

# Re-export for backwards compatibility with existing tests
__all__ = [
    "ScenarioContext",
    "uuid_for",
    "uuid_str_for",
    "make_timestamp",
    "pack_event",
    "make_cover",
    "make_event_book",
    "make_command_book",
]


@pytest.fixture
def context():
    """Create a fresh test context."""
    return ScenarioContext()


@pytest.fixture
def player_root():
    """Default player root for tests."""
    return uuid_for("player-test")


@pytest.fixture
def table_root():
    """Default table root for tests."""
    return uuid_for("table-test")


@pytest.fixture
def hand_root():
    """Default hand root for tests."""
    return uuid_for("hand-test")


# Re-export commonly used modules
@pytest.fixture
def player_pb():
    """Player protobuf module."""
    return player


@pytest.fixture
def table_pb():
    """Table protobuf module."""
    return table


@pytest.fixture
def hand_pb():
    """Hand protobuf module."""
    return hand


@pytest.fixture
def poker_types_pb():
    """Poker types protobuf module."""
    return poker_types
