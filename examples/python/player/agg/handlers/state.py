"""Player state management for router pattern.

Provides PlayerState dataclass and StateRouter for event sourcing.
"""

from dataclasses import dataclass, field

from google.protobuf.any_pb2 import Any as AnyProto

from angzarr_client.proto.examples import player_pb2 as player_proto
from angzarr_client.state_builder import StateRouter


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


# docs:start:state_router
def _apply_registered(state: PlayerState, event: player_proto.PlayerRegistered) -> None:
    state.player_id = f"player_{event.email}"
    state.display_name = event.display_name
    state.email = event.email
    state.player_type = event.player_type
    state.ai_model_id = event.ai_model_id
    state.status = "active"
    state.bankroll = 0
    state.reserved_funds = 0


# Why events carry final state (not deltas):
# Events contain new_balance (the result) rather than delta (amount deposited).
# This provides: (1) idempotent replay, (2) auditable history, (3) simpler appliers.
def _apply_deposited(state: PlayerState, event: player_proto.FundsDeposited) -> None:
    if event.new_balance:
        state.bankroll = event.new_balance.amount


def _apply_withdrawn(state: PlayerState, event: player_proto.FundsWithdrawn) -> None:
    if event.new_balance:
        state.bankroll = event.new_balance.amount


def _apply_reserved(state: PlayerState, event: player_proto.FundsReserved) -> None:
    if event.new_reserved_balance:
        state.reserved_funds = event.new_reserved_balance.amount
    table_key = event.table_root.hex()
    if event.amount:
        state.table_reservations[table_key] = event.amount.amount


def _apply_released(state: PlayerState, event: player_proto.FundsReleased) -> None:
    if event.new_reserved_balance:
        state.reserved_funds = event.new_reserved_balance.amount
    table_key = event.table_root.hex()
    state.table_reservations.pop(table_key, None)


def _apply_transferred(
    state: PlayerState, event: player_proto.FundsTransferred
) -> None:
    if event.new_balance:
        state.bankroll = event.new_balance.amount


player_state_router = (
    StateRouter(PlayerState)
    .on(player_proto.PlayerRegistered, _apply_registered)
    .on(player_proto.FundsDeposited, _apply_deposited)
    .on(player_proto.FundsWithdrawn, _apply_withdrawn)
    .on(player_proto.FundsReserved, _apply_reserved)
    .on(player_proto.FundsReleased, _apply_released)
    .on(player_proto.FundsTransferred, _apply_transferred)
)
# docs:end:state_router


def build_state(state: PlayerState, events: list[AnyProto]) -> PlayerState:
    """Apply events to state and return updated state."""
    for event_any in events:
        player_state_router.apply_single(state, event_any)
    return state


__all__ = ["PlayerState", "build_state", "player_state_router"]
