"""SagaContext for splitter pattern support.

SagaContext provides convenient access to destination aggregate state
in the splitter pattern, where one event triggers commands to multiple aggregates.

Example usage:
    def handle_table_settled(event: TableSettled, destinations: list[EventBook]) -> list[CommandBook]:
        ctx = SagaContext(destinations)
        commands = []

        for payout in event.payouts:
            seq = ctx.get_sequence("player", payout.player_root)
            cmd = TransferFunds(player_root=payout.player_root, amount=payout.amount)
            commands.append(new_command_book("player", cmd, sequence=seq))

        return commands
"""

from __future__ import annotations

from .helpers import next_sequence, root_id_hex
from .proto.angzarr import types_pb2 as types

__all__ = ["SagaContext"]


class SagaContext:
    """Context for saga handlers providing access to destination aggregate state.

    Used in the splitter pattern where one event triggers commands to multiple
    aggregates. Provides sequence number lookup for optimistic concurrency control.
    """

    def __init__(self, destinations: list[types.EventBook]) -> None:
        """Create a context from a list of destination EventBooks.

        Args:
            destinations: List of EventBooks fetched during prepare phase.
        """
        self._destinations: dict[str, types.EventBook] = {}
        for book in destinations:
            if book.HasField("cover") and book.cover.domain:
                key = self._make_key(book.cover.domain, book.cover.root.value)
                self._destinations[key] = book

    def get_sequence(self, domain: str, aggregate_root: bytes) -> int:
        """Get the next sequence number for a destination aggregate.

        Returns 1 if the aggregate doesn't exist yet.

        Args:
            domain: The domain of the target aggregate.
            aggregate_root: The root identifier (as bytes).

        Returns:
            The next sequence number for the aggregate.
        """
        key = self._make_key(domain, aggregate_root)
        book = self._destinations.get(key)
        if book is None:
            return 1
        return next_sequence(book)

    def get_destination(
        self, domain: str, aggregate_root: bytes
    ) -> types.EventBook | None:
        """Get the EventBook for a destination aggregate.

        Args:
            domain: The domain of the target aggregate.
            aggregate_root: The root identifier (as bytes).

        Returns:
            The EventBook if found, None otherwise.
        """
        key = self._make_key(domain, aggregate_root)
        return self._destinations.get(key)

    def has_destination(self, domain: str, aggregate_root: bytes) -> bool:
        """Check if a destination exists.

        Args:
            domain: The domain of the target aggregate.
            aggregate_root: The root identifier (as bytes).

        Returns:
            True if the destination exists.
        """
        key = self._make_key(domain, aggregate_root)
        return key in self._destinations

    @staticmethod
    def _make_key(domain: str, root: bytes) -> str:
        """Create a lookup key from domain and root."""
        return f"{domain}:{root.hex()}"
