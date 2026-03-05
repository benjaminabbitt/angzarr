"""Player aggregate package (functional handler pattern)."""

from .agg.handlers import (
    handle_deposit,
    handle_register,
    handle_release,
    handle_reserve,
    handle_withdraw,
)
from .agg.state import PlayerState, build_state

__all__ = [
    "PlayerState",
    "build_state",
    "handle_deposit",
    "handle_register",
    "handle_release",
    "handle_reserve",
    "handle_withdraw",
]
