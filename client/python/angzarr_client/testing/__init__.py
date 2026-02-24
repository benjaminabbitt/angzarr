"""Testing utilities for angzarr applications.

This module provides helpers for writing tests against angzarr aggregates,
sagas, and other components. It includes:

- **UUID generation**: Deterministic UUID creation for reproducible tests
- **Proto builders**: Simplified constructors for EventBook, CommandBook, etc.
- **Scenario context**: BDD-style context for tracking test state

Example usage:
    from angzarr_client.testing import (
        ScenarioContext,
        uuid_for,
        make_cover,
        make_event_book,
        pack_event,
    )

    def test_player_registration():
        ctx = ScenarioContext()
        ctx.domain = "player"
        ctx.root = uuid_for("player-alice")

        # Build event book with prior events
        book = ctx.event_book()

        # Execute command and verify
        result = handler.handle(cmd, book)
        assert result.player_id == "player_alice@test.com"
"""

from .builders import (
    make_command_book,
    make_command_page,
    make_cover,
    make_event_book,
    make_event_page,
    make_timestamp,
    pack_event,
)
from .context import ScenarioContext
from .uuid import (
    DEFAULT_TEST_NAMESPACE,
    uuid_for,
    uuid_obj_for,
    uuid_str_for,
)

__all__ = [
    # UUID helpers
    "DEFAULT_TEST_NAMESPACE",
    "uuid_for",
    "uuid_str_for",
    "uuid_obj_for",
    # Proto builders
    "make_timestamp",
    "pack_event",
    "make_cover",
    "make_event_page",
    "make_event_book",
    "make_command_page",
    "make_command_book",
    # Context
    "ScenarioContext",
]
