"""Shared pytest fixtures for poker tests."""

import sys
from pathlib import Path
from dataclasses import dataclass, field
from typing import Any, Optional
from uuid import UUID, uuid5, NAMESPACE_DNS

import pytest
from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.timestamp_pb2 import Timestamp

# Add paths for imports
root = Path(__file__).parent.parent
sys.path.insert(0, str(root))
sys.path.insert(0, str(root / "player" / "agg"))
sys.path.insert(0, str(root / "table" / "agg"))
sys.path.insert(0, str(root / "hand" / "agg"))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import poker_types_pb2 as poker_types
from angzarr_client.errors import CommandRejectedError


# Namespace for test UUIDs - consistent across all tests
TEST_UUID_NAMESPACE = UUID("a1b2c3d4-e5f6-7890-abcd-ef1234567890")


def uuid_for(name: str) -> bytes:
    """Generate a deterministic 16-byte UUID from a name.

    Always returns a proper UUID that will display as standard text format.
    The same name always generates the same UUID.
    """
    return uuid5(TEST_UUID_NAMESPACE, name).bytes


def uuid_str_for(name: str) -> str:
    """Generate a deterministic UUID string from a name."""
    return str(uuid5(TEST_UUID_NAMESPACE, name))


def make_timestamp() -> Timestamp:
    """Create a timestamp for now."""
    from datetime import datetime, timezone
    return Timestamp(seconds=int(datetime.now(timezone.utc).timestamp()))


def pack_event(msg, type_url_prefix: str = "type.poker/") -> ProtoAny:
    """Pack a protobuf message into Any."""
    event_any = ProtoAny()
    event_any.Pack(msg, type_url_prefix=type_url_prefix)
    return event_any


def make_cover(domain: str, root: bytes) -> types.Cover:
    """Create a Cover."""
    return types.Cover(domain=domain, root=types.UUID(value=root))


def make_event_book(cover: types.Cover, pages: list = None) -> types.EventBook:
    """Create an EventBook."""
    return types.EventBook(
        cover=cover,
        pages=pages or [],
        next_sequence=len(pages) if pages else 0,
    )


def make_command_book(cover: types.Cover, command_any: ProtoAny, seq: int = 0) -> types.CommandBook:
    """Create a CommandBook."""
    return types.CommandBook(
        cover=cover,
        pages=[
            types.CommandPage(
                sequence=seq,
                command=command_any,
            )
        ],
    )


@dataclass
class ScenarioContext:
    """Shared context for test scenarios."""

    # Current aggregate being tested
    domain: str = ""
    root: bytes = b""

    # Event history
    events: list = field(default_factory=list)

    # Last command result
    result: Any = None
    error: Optional[CommandRejectedError] = None

    # State after rebuild
    state: Any = None

    def event_book(self) -> types.EventBook:
        """Build EventBook from events."""
        pages = []
        for i, event_any in enumerate(self.events):
            pages.append(types.EventPage(
                num=i,
                event=event_any,
                created_at=make_timestamp(),
            ))
        return types.EventBook(
            cover=make_cover(self.domain, self.root),
            pages=pages,
            next_sequence=len(pages),
        )

    def add_event(self, event_msg, type_url_prefix: str = "type.poker/"):
        """Add an event to history."""
        self.events.append(pack_event(event_msg, type_url_prefix))


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
