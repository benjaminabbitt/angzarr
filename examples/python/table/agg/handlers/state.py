"""Table state management."""

import sys
from pathlib import Path
from dataclasses import dataclass, field
from typing import Optional

# Add path for proto imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import types_pb2 as types


@dataclass
class SeatState:
    """State of a single seat at the table."""

    position: int
    player_root: bytes
    stack: int
    is_active: bool = True
    is_sitting_out: bool = False


@dataclass
class TableState:
    """Table aggregate state."""

    table_id: str = ""
    table_name: str = ""
    game_variant: int = 0
    small_blind: int = 0
    big_blind: int = 0
    min_buy_in: int = 0
    max_buy_in: int = 0
    max_players: int = 9
    action_timeout_seconds: int = 30
    seats: dict = field(default_factory=dict)  # position -> SeatState
    dealer_position: int = 0
    hand_count: int = 0
    current_hand_root: bytes = b""
    status: str = ""  # "waiting", "in_hand", "paused"

    def exists(self) -> bool:
        """Check if table exists."""
        return bool(self.table_id)

    def player_count(self) -> int:
        """Get number of seated players."""
        return len(self.seats)

    def active_player_count(self) -> int:
        """Get number of active (not sitting out) players."""
        return sum(1 for s in self.seats.values() if not s.is_sitting_out)

    def is_full(self) -> bool:
        """Check if table is full."""
        return self.player_count() >= self.max_players

    def get_seat(self, position: int) -> Optional[SeatState]:
        """Get seat at position."""
        return self.seats.get(position)

    def find_player_seat(self, player_root: bytes) -> Optional[SeatState]:
        """Find seat occupied by player."""
        for seat in self.seats.values():
            if seat.player_root == player_root:
                return seat
        return None

    def find_available_seat(self, preferred: int = -1) -> Optional[int]:
        """Find an available seat position."""
        if preferred >= 0 and preferred < self.max_players:
            if preferred not in self.seats:
                return preferred

        for pos in range(self.max_players):
            if pos not in self.seats:
                return pos
        return None

    def next_dealer_position(self) -> int:
        """Get next dealer button position."""
        if not self.seats:
            return 0

        positions = sorted(self.seats.keys())
        current_idx = 0
        for i, pos in enumerate(positions):
            if pos == self.dealer_position:
                current_idx = i
                break

        next_idx = (current_idx + 1) % len(positions)
        return positions[next_idx]


def rebuild_state(event_book) -> TableState:
    """Rebuild table state from event history."""
    state = TableState()

    if event_book is None:
        return state

    # Start from snapshot if available
    if event_book.snapshot and event_book.snapshot.state:
        snapshot = table.TableState()
        if event_book.snapshot.state.Is(table.TableState.DESCRIPTOR):
            event_book.snapshot.state.Unpack(snapshot)
            state = _apply_snapshot(snapshot)

    # Apply events since snapshot
    for page in event_book.pages:
        if page.event:
            _apply_event(state, page.event)

    return state


def _apply_snapshot(snapshot: table.TableState) -> TableState:
    """Create state from snapshot."""
    state = TableState(
        table_id=snapshot.table_id,
        table_name=snapshot.table_name,
        game_variant=snapshot.game_variant,
        small_blind=snapshot.small_blind,
        big_blind=snapshot.big_blind,
        min_buy_in=snapshot.min_buy_in,
        max_buy_in=snapshot.max_buy_in,
        max_players=snapshot.max_players,
        action_timeout_seconds=snapshot.action_timeout_seconds,
        dealer_position=snapshot.dealer_position,
        hand_count=snapshot.hand_count,
        current_hand_root=snapshot.current_hand_root,
        status=snapshot.status,
    )

    for seat in snapshot.seats:
        state.seats[seat.position] = SeatState(
            position=seat.position,
            player_root=seat.player_root,
            stack=seat.stack.amount if seat.stack else 0,
            is_active=seat.is_active,
            is_sitting_out=seat.is_sitting_out,
        )

    return state


def _apply_event(state: TableState, event_any) -> None:
    """Apply a single event to state."""
    type_url = event_any.type_url

    if type_url.endswith("TableCreated"):
        event = table.TableCreated()
        event_any.Unpack(event)
        state.table_id = f"table_{event.table_name}"
        state.table_name = event.table_name
        state.game_variant = event.game_variant
        state.small_blind = event.small_blind
        state.big_blind = event.big_blind
        state.min_buy_in = event.min_buy_in
        state.max_buy_in = event.max_buy_in
        state.max_players = event.max_players
        state.action_timeout_seconds = event.action_timeout_seconds
        state.status = "waiting"

    elif type_url.endswith("PlayerJoined"):
        event = table.PlayerJoined()
        event_any.Unpack(event)
        state.seats[event.seat_position] = SeatState(
            position=event.seat_position,
            player_root=event.player_root,
            stack=event.stack,
        )

    elif type_url.endswith("PlayerLeft"):
        event = table.PlayerLeft()
        event_any.Unpack(event)
        state.seats.pop(event.seat_position, None)

    elif type_url.endswith("PlayerSatOut"):
        event = table.PlayerSatOut()
        event_any.Unpack(event)
        for seat in state.seats.values():
            if seat.player_root == event.player_root:
                seat.is_sitting_out = True
                break

    elif type_url.endswith("PlayerSatIn"):
        event = table.PlayerSatIn()
        event_any.Unpack(event)
        for seat in state.seats.values():
            if seat.player_root == event.player_root:
                seat.is_sitting_out = False
                break

    elif type_url.endswith("HandStarted"):
        event = table.HandStarted()
        event_any.Unpack(event)
        state.hand_count = event.hand_number
        state.current_hand_root = event.hand_root
        state.dealer_position = event.dealer_position
        state.status = "in_hand"

    elif type_url.endswith("HandEnded"):
        event = table.HandEnded()
        event_any.Unpack(event)
        state.current_hand_root = b""
        state.status = "waiting"
        # Apply stack changes
        for player_hex, delta in event.stack_changes.items():
            player_root = bytes.fromhex(player_hex)
            for seat in state.seats.values():
                if seat.player_root == player_root:
                    seat.stack += delta
                    break

    elif type_url.endswith("ChipsAdded"):
        event = table.ChipsAdded()
        event_any.Unpack(event)
        for seat in state.seats.values():
            if seat.player_root == event.player_root:
                seat.stack = event.new_stack
                break
