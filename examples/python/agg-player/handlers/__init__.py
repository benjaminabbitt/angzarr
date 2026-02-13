"""Command handlers for Player bounded context."""

import sys
from pathlib import Path

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.errors import CommandRejectedError

from .register_player import handle_register_player
from .deposit_funds import handle_deposit_funds
from .withdraw_funds import handle_withdraw_funds
from .reserve_funds import handle_reserve_funds
from .release_funds import handle_release_funds
from .request_action import handle_request_action
from .ai_client import ActionDecider, HumanActionDecider, AiActionDecider, get_decider

__all__ = [
    "CommandRejectedError",
    "handle_register_player",
    "handle_deposit_funds",
    "handle_withdraw_funds",
    "handle_reserve_funds",
    "handle_release_funds",
    "handle_request_action",
    "ActionDecider",
    "HumanActionDecider",
    "AiActionDecider",
    "get_decider",
]
