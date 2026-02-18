"""Player state - functional pattern using StateRouter.

This file defines the player state and event appliers as pure functions.
Contrasts with the OO pattern in player/agg/handlers/player.py which
uses decorators on class methods.
"""

from dataclasses import dataclass, field

from angzarr_client.proto.examples import player_pb2 as player


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


# --- Event appliers (pure functions) ---


def apply_registered(state: PlayerState, event: player.PlayerRegistered) -> None:
    """Apply PlayerRegistered event to state."""
    state.player_id = f"player_{event.email}"
    state.display_name = event.display_name
    state.email = event.email
    state.player_type = event.player_type
    state.ai_model_id = event.ai_model_id
    state.status = "active"
    state.bankroll = 0
    state.reserved_funds = 0


def apply_deposited(state: PlayerState, event: player.FundsDeposited) -> None:
    """Apply FundsDeposited event to state."""
    if event.new_balance:
        state.bankroll = event.new_balance.amount


def apply_withdrawn(state: PlayerState, event: player.FundsWithdrawn) -> None:
    """Apply FundsWithdrawn event to state."""
    if event.new_balance:
        state.bankroll = event.new_balance.amount


def apply_reserved(state: PlayerState, event: player.FundsReserved) -> None:
    """Apply FundsReserved event to state."""
    if event.new_reserved_balance:
        state.reserved_funds = event.new_reserved_balance.amount
    table_key = event.table_root.hex()
    if event.amount:
        state.table_reservations[table_key] = event.amount.amount


def apply_released(state: PlayerState, event: player.FundsReleased) -> None:
    """Apply FundsReleased event to state."""
    if event.new_reserved_balance:
        state.reserved_funds = event.new_reserved_balance.amount
    table_key = event.table_root.hex()
    state.table_reservations.pop(table_key, None)


def apply_transferred(state: PlayerState, event: player.FundsTransferred) -> None:
    """Apply FundsTransferred event to state."""
    if event.new_balance:
        state.bankroll = event.new_balance.amount
