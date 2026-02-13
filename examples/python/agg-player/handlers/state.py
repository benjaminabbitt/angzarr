"""Player state management."""

import sys
from pathlib import Path
from dataclasses import dataclass, field
from typing import Optional

# Add path for proto imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.examples import player_pb2 as player


@dataclass
class PlayerState:
    """Player aggregate state."""

    player_id: str = ""
    display_name: str = ""
    email: str = ""
    player_type: int = 0  # PlayerType enum
    ai_model_id: str = ""
    bankroll: int = 0  # In smallest unit (chips)
    reserved_funds: int = 0
    table_reservations: dict = field(default_factory=dict)  # table_root_hex -> amount
    status: str = ""

    def exists(self) -> bool:
        """Check if player exists (has been registered)."""
        return bool(self.player_id)

    def available_balance(self) -> int:
        """Get available balance (bankroll - reserved)."""
        return self.bankroll - self.reserved_funds

    def is_ai(self) -> bool:
        """Check if this is an AI player."""
        return self.player_type == player.PlayerType.AI


def rebuild_state(event_book) -> PlayerState:
    """Rebuild player state from event history."""
    state = PlayerState()

    if event_book is None:
        return state

    # Start from snapshot if available
    if event_book.snapshot and event_book.snapshot.state:
        snapshot = player.PlayerState()
        if event_book.snapshot.state.Is(player.PlayerState.DESCRIPTOR):
            event_book.snapshot.state.Unpack(snapshot)
            state = _apply_snapshot(snapshot)

    # Apply events since snapshot
    for page in event_book.pages:
        if page.event:
            _apply_event(state, page.event)

    return state


def _apply_snapshot(snapshot: player.PlayerState) -> PlayerState:
    """Create state from snapshot."""
    return PlayerState(
        player_id=snapshot.player_id,
        display_name=snapshot.display_name,
        email=snapshot.email,
        player_type=snapshot.player_type,
        ai_model_id=snapshot.ai_model_id,
        bankroll=snapshot.bankroll.amount if snapshot.bankroll else 0,
        reserved_funds=snapshot.reserved_funds.amount if snapshot.reserved_funds else 0,
        table_reservations=dict(snapshot.table_reservations),
        status=snapshot.status,
    )


def _apply_event(state: PlayerState, event_any) -> None:
    """Apply a single event to state."""
    type_url = event_any.type_url

    if type_url.endswith("PlayerRegistered"):
        event = player.PlayerRegistered()
        event_any.Unpack(event)
        state.player_id = f"player_{event.email}"
        state.display_name = event.display_name
        state.email = event.email
        state.player_type = event.player_type
        state.ai_model_id = event.ai_model_id
        state.status = "active"
        state.bankroll = 0
        state.reserved_funds = 0

    elif type_url.endswith("FundsDeposited"):
        event = player.FundsDeposited()
        event_any.Unpack(event)
        if event.new_balance:
            state.bankroll = event.new_balance.amount

    elif type_url.endswith("FundsWithdrawn"):
        event = player.FundsWithdrawn()
        event_any.Unpack(event)
        if event.new_balance:
            state.bankroll = event.new_balance.amount

    elif type_url.endswith("FundsReserved"):
        event = player.FundsReserved()
        event_any.Unpack(event)
        if event.new_reserved_balance:
            state.reserved_funds = event.new_reserved_balance.amount
        table_key = event.table_root.hex()
        if event.amount:
            state.table_reservations[table_key] = event.amount.amount

    elif type_url.endswith("FundsReleased"):
        event = player.FundsReleased()
        event_any.Unpack(event)
        if event.new_reserved_balance:
            state.reserved_funds = event.new_reserved_balance.amount
        table_key = event.table_root.hex()
        state.table_reservations.pop(table_key, None)

    elif type_url.endswith("FundsTransferred"):
        event = player.FundsTransferred()
        event_any.Unpack(event)
        if event.new_balance:
            state.bankroll = event.new_balance.amount
