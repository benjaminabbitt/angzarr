"""Saga unit tests.

Note: These tests require the sagas package which is not yet implemented.
Tests are skipped if the module is not available.
"""

import sys
from pathlib import Path

import pytest
from pytest_bdd import scenarios, given, when, then, parsers
from google.protobuf.any_pb2 import Any as ProtoAny

# Add paths
root = Path(__file__).parent.parent.parent
sys.path.insert(0, str(root))
sys.path.insert(0, str(root / "sagas"))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

try:
    from sagas.base import Saga, SagaContext, SagaRouter
    from sagas.table_sync_saga import TableSyncSaga
    from sagas.hand_results_saga import HandResultsSaga

    SAGAS_AVAILABLE = True
except ImportError:
    SAGAS_AVAILABLE = False
    Saga = None
    SagaContext = None
    SagaRouter = None
    TableSyncSaga = None
    HandResultsSaga = None

from tests.conftest import make_cover, make_timestamp, pack_event, uuid_for

# Skip all tests in this module if sagas not available
pytestmark = pytest.mark.skipif(
    not SAGAS_AVAILABLE, reason="sagas module not implemented"
)


# --- Test context ---


class SagaTestContext:
    """Test context for saga scenarios."""

    def __init__(self):
        self.saga = None
        self.router = None
        self.event_book = None
        self.commands = []
        self.error = None
        self._event = None


@pytest.fixture
def ctx():
    """Saga test context."""
    return SagaTestContext()


# --- Helper functions ---


def make_event_book(domain: str, root: bytes, events: list) -> types.EventBook:
    """Create an EventBook with events."""
    pages = []
    for i, event_any in enumerate(events):
        pages.append(
            types.EventPage(
                num=i,
                event=event_any,
                created_at=make_timestamp(),
            )
        )
    return types.EventBook(
        cover=make_cover(domain, root),
        pages=pages,
        next_sequence=len(pages),
    )


def _extract_event_type(type_url: str) -> str:
    """Extract event type from type_url."""
    if "." in type_url:
        return type_url.split(".")[-1]
    return type_url


# --- Unit tests (not using feature files due to datatable limitations) ---


class TestTableSyncSaga:
    """Tests for TableSyncSaga."""

    def test_routes_hand_started_to_deal_cards(self):
        """TableSyncSaga should emit DealCards when HandStarted is received."""
        saga = TableSyncSaga()

        # Create HandStarted event
        event = table.HandStarted(
            hand_root=uuid_for("hand-1"),
            hand_number=1,
            game_variant=poker_types.TEXAS_HOLDEM,
            dealer_position=0,
            small_blind=100,
            big_blind=200,
        )
        event.active_players.append(
            table.SeatSnapshot(player_root=uuid_for("player-1"), position=0, stack=500)
        )
        event.active_players.append(
            table.SeatSnapshot(player_root=uuid_for("player-2"), position=1, stack=500)
        )

        event_any = pack_event(event)
        event_book = make_event_book("table", uuid_for("table-1"), [event_any])

        context = SagaContext(
            event_book=event_book,
            event_type="HandStarted",
            aggregate_type="table",
            aggregate_root=uuid_for("table-1"),
        )

        commands = saga.handle(context)

        assert len(commands) == 1
        assert commands[0].cover.domain == "hand"
        assert commands[0].pages[0].command.type_url.endswith("DealCards")

        # Verify command content
        cmd = hand.DealCards()
        commands[0].pages[0].command.Unpack(cmd)
        assert cmd.game_variant == poker_types.TEXAS_HOLDEM
        assert len(cmd.players) == 2
        assert cmd.hand_number == 1

    def test_routes_hand_complete_to_end_hand(self):
        """TableSyncSaga should emit EndHand when HandComplete is received."""
        saga = TableSyncSaga()

        # Create HandComplete event
        event = hand.HandComplete(
            table_root=uuid_for("table-1"),
            hand_number=1,
        )
        event.winners.append(
            hand.PotWinner(player_root=uuid_for("player-1"), amount=100)
        )

        event_any = pack_event(event)
        event_book = make_event_book("hand", uuid_for("hand-1"), [event_any])

        context = SagaContext(
            event_book=event_book,
            event_type="HandComplete",
            aggregate_type="hand",
            aggregate_root=uuid_for("hand-1"),
        )

        commands = saga.handle(context)

        assert len(commands) == 1
        assert commands[0].cover.domain == "table"
        assert commands[0].pages[0].command.type_url.endswith("EndHand")

        # Verify command content
        cmd = table.EndHand()
        commands[0].pages[0].command.Unpack(cmd)
        assert len(cmd.results) == 1
        assert cmd.results[0].winner_root == uuid_for("player-1")
        assert cmd.results[0].amount == 100


class TestHandResultsSaga:
    """Tests for HandResultsSaga."""

    def test_routes_hand_ended_to_release_funds(self):
        """HandResultsSaga should emit ReleaseFunds when HandEnded is received."""
        saga = HandResultsSaga()

        # Create HandEnded event
        event = table.HandEnded(
            hand_root=uuid_for("hand-1"),
        )
        # Add stack changes for two players (map keys are hex-encoded UUIDs)
        event.stack_changes[uuid_for("player-1").hex()] = 50
        event.stack_changes[uuid_for("player-2").hex()] = -50

        event_any = pack_event(event)
        event_book = make_event_book("table", uuid_for("table-1"), [event_any])

        context = SagaContext(
            event_book=event_book,
            event_type="HandEnded",
            aggregate_type="table",
            aggregate_root=uuid_for("table-1"),
        )

        commands = saga.handle(context)

        assert len(commands) == 2
        for cmd_book in commands:
            assert cmd_book.cover.domain == "player"
            assert cmd_book.pages[0].command.type_url.endswith("ReleaseFunds")

    def test_routes_pot_awarded_to_deposit_funds(self):
        """HandResultsSaga should emit DepositFunds when PotAwarded is received."""
        saga = HandResultsSaga()

        # Create PotAwarded event
        event = hand.PotAwarded()
        event.winners.append(
            hand.PotWinner(player_root=uuid_for("player-1"), amount=60)
        )
        event.winners.append(
            hand.PotWinner(player_root=uuid_for("player-2"), amount=40)
        )

        event_any = pack_event(event)
        event_book = make_event_book("hand", uuid_for("hand-1"), [event_any])

        context = SagaContext(
            event_book=event_book,
            event_type="PotAwarded",
            aggregate_type="hand",
            aggregate_root=uuid_for("hand-1"),
        )

        commands = saga.handle(context)

        assert len(commands) == 2
        for cmd_book in commands:
            assert cmd_book.cover.domain == "player"
            assert cmd_book.pages[0].command.type_url.endswith("DepositFunds")

        # Verify first command
        cmd1 = player.DepositFunds()
        commands[0].pages[0].command.Unpack(cmd1)
        assert cmd1.amount.amount == 60
        assert commands[0].cover.root.value == uuid_for("player-1")

        # Verify second command
        cmd2 = player.DepositFunds()
        commands[1].pages[0].command.Unpack(cmd2)
        assert cmd2.amount.amount == 40
        assert commands[1].cover.root.value == uuid_for("player-2")


class TestSagaRouter:
    """Tests for SagaRouter."""

    def test_dispatches_to_matching_sagas_only(self):
        """SagaRouter should dispatch events to sagas that subscribe to them."""
        router = SagaRouter()
        router.register(TableSyncSaga())
        router.register(HandResultsSaga())

        # Create HandStarted event (only TableSyncSaga handles this)
        event = table.HandStarted(
            hand_root=uuid_for("hand-1"),
            hand_number=1,
            game_variant=poker_types.TEXAS_HOLDEM,
        )
        event.active_players.append(
            table.SeatSnapshot(player_root=uuid_for("player-1"), position=0, stack=500)
        )

        event_any = pack_event(event)
        event_book = make_event_book("table", uuid_for("table-1"), [event_any])

        commands = router.route(event_book, "table")

        # Only DealCards from TableSyncSaga
        assert len(commands) == 1
        assert commands[0].pages[0].command.type_url.endswith("DealCards")

    def test_handles_multiple_events_in_event_book(self):
        """SagaRouter should handle multiple events in an event book."""
        router = SagaRouter()
        router.register(TableSyncSaga())

        # Create multiple HandStarted events
        events = []
        for i in range(2):
            event = table.HandStarted(
                hand_root=uuid_for(f"hand-{i}"),
                hand_number=i + 1,
                game_variant=poker_types.TEXAS_HOLDEM,
            )
            event.active_players.append(
                table.SeatSnapshot(
                    player_root=uuid_for("player-1"), position=0, stack=500
                )
            )
            events.append(pack_event(event))

        event_book = make_event_book("table", uuid_for("table-1"), events)

        commands = router.route(event_book, "table")

        assert len(commands) == 2
        for cmd in commands:
            assert cmd.pages[0].command.type_url.endswith("DealCards")

    def test_continues_after_saga_failure(self):
        """SagaRouter should continue processing after a saga fails."""

        class FailingSaga(Saga):
            @property
            def name(self):
                return "FailingSaga"

            @property
            def subscribed_events(self):
                return ["HandStarted"]

            def handle(self, context):
                raise RuntimeError("Saga failure")

        router = SagaRouter()
        router.register(FailingSaga())
        router.register(TableSyncSaga())

        event = table.HandStarted(
            hand_root=uuid_for("hand-1"),
            hand_number=1,
            game_variant=poker_types.TEXAS_HOLDEM,
        )
        event.active_players.append(
            table.SeatSnapshot(player_root=uuid_for("player-1"), position=0, stack=500)
        )

        event_any = pack_event(event)
        event_book = make_event_book("table", uuid_for("table-1"), [event_any])

        # Should not raise exception
        commands = router.route(event_book, "table")

        # TableSyncSaga should still emit its command
        deal_commands = [
            cmd
            for cmd in commands
            if cmd.pages[0].command.type_url.endswith("DealCards")
        ]
        assert len(deal_commands) == 1
