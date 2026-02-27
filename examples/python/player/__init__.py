"""Player aggregate package (functional handler pattern)."""

from .agg.handlers import (
    handle_deposit_funds,
    handle_register_player,
    handle_release_funds,
    handle_reserve_funds,
    handle_sit_in,
    handle_sit_out,
    handle_withdraw_funds,
)
from .agg.handlers.state import PlayerState, build_state

__all__ = [
    "PlayerState",
    "build_state",
    "handle_deposit_funds",
    "handle_register_player",
    "handle_release_funds",
    "handle_reserve_funds",
    "handle_sit_out",
    "handle_sit_in",
    "handle_withdraw_funds",
]
