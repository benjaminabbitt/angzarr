"""Player state management for router pattern.

Provides PlayerState dataclass and build_state function for event sourcing.
"""

from dataclasses import dataclass, field

from google.protobuf.any_pb2 import Any as AnyProto

from angzarr_client.helpers import try_unpack
from angzarr_client.proto.examples import player_pb2 as player_proto


@dataclass
class PlayerState:
    """Player aggregate state."""

    player_id: str = ""
    display_name: str = ""
    email: str = ""
    player_type: int = 0
    ai_model_id: str = ""
    bankroll: int = 0
    reserved_funds: int = 0
    table_reservations: dict = field(default_factory=dict)
    status: str = ""

    @property
    def exists(self) -> bool:
        return bool(self.player_id)

    @property
    def available_balance(self) -> int:
        return self.bankroll - self.reserved_funds


def _apply_event(state: PlayerState, event_any: AnyProto) -> None:
    """Apply a single event to state (mutates in place)."""
    if event := try_unpack(event_any, player_proto.PlayerRegistered):
        state.player_id = f"player_{event.email}"
        state.display_name = event.display_name
        state.email = event.email
        state.player_type = event.player_type
        state.ai_model_id = event.ai_model_id
        state.status = "active"
        state.bankroll = 0
        state.reserved_funds = 0

    elif event := try_unpack(event_any, player_proto.FundsDeposited):
        if event.new_balance:
            state.bankroll = event.new_balance.amount

    elif event := try_unpack(event_any, player_proto.FundsWithdrawn):
        if event.new_balance:
            state.bankroll = event.new_balance.amount

    elif event := try_unpack(event_any, player_proto.FundsReserved):
        if event.new_reserved_balance:
            state.reserved_funds = event.new_reserved_balance.amount
        table_key = event.table_root.hex()
        if event.amount:
            state.table_reservations[table_key] = event.amount.amount

    elif event := try_unpack(event_any, player_proto.FundsReleased):
        if event.new_reserved_balance:
            state.reserved_funds = event.new_reserved_balance.amount
        table_key = event.table_root.hex()
        state.table_reservations.pop(table_key, None)

    elif event := try_unpack(event_any, player_proto.FundsTransferred):
        if event.new_balance:
            state.bankroll = event.new_balance.amount


def build_state(state: PlayerState, events: list[AnyProto]) -> PlayerState:
    """Apply events to state and return updated state."""
    for event_any in events:
        _apply_event(state, event_any)
    return state


__all__ = ["PlayerState", "build_state"]
