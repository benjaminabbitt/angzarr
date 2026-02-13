"""Handler for PlayerAction command."""

import sys
from pathlib import Path
from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import types_pb2 as poker_types

from .state import HandState


def handle_player_action(
    command_book: types.CommandBook,
    command_any: Any,
    state: HandState,
    seq: int,
) -> types.EventBook:
    """Handle PlayerAction command."""
    if not state.exists():
        raise CommandRejectedError("Hand not dealt")

    if state.status != "betting":
        raise CommandRejectedError("Not in betting phase")

    cmd = hand.PlayerAction()
    command_any.Unpack(cmd)

    if not cmd.player_root:
        raise CommandRejectedError("player_root is required")

    # Find the player
    player = None
    for p in state.players.values():
        if p.player_root == cmd.player_root:
            player = p
            break

    if not player:
        raise CommandRejectedError("Player not in hand")

    if player.has_folded:
        raise CommandRejectedError("Player has folded")

    if player.is_all_in:
        raise CommandRejectedError("Player is all-in")

    # Validate action
    action = cmd.action
    amount = cmd.amount
    call_amount = state.current_bet - player.bet_this_round

    if action == poker_types.FOLD:
        # Fold is always valid
        new_stack = player.stack
        amount = 0

    elif action == poker_types.CHECK:
        if call_amount > 0:
            raise CommandRejectedError("Cannot check when there is a bet to call")
        new_stack = player.stack
        amount = 0

    elif action == poker_types.CALL:
        if call_amount == 0:
            raise CommandRejectedError("Nothing to call")
        actual_amount = min(call_amount, player.stack)
        new_stack = player.stack - actual_amount
        amount = actual_amount
        if new_stack == 0:
            action = poker_types.ALL_IN

    elif action == poker_types.BET:
        if state.current_bet > 0:
            raise CommandRejectedError("Cannot bet when there is already a bet")
        if amount < state.big_blind:
            raise CommandRejectedError(f"Bet must be at least {state.big_blind}")
        if amount > player.stack:
            raise CommandRejectedError("Bet exceeds stack")
        new_stack = player.stack - amount
        if new_stack == 0:
            action = poker_types.ALL_IN

    elif action == poker_types.RAISE:
        if state.current_bet == 0:
            raise CommandRejectedError("Cannot raise when there is no bet")
        total_bet = player.bet_this_round + amount
        raise_amount = total_bet - state.current_bet
        if raise_amount < state.min_raise and amount < player.stack:
            raise CommandRejectedError(f"Raise must be at least {state.min_raise}")
        if amount > player.stack:
            raise CommandRejectedError("Raise exceeds stack")
        new_stack = player.stack - amount
        if new_stack == 0:
            action = poker_types.ALL_IN

    elif action == poker_types.ALL_IN:
        amount = player.stack
        new_stack = 0

    else:
        raise CommandRejectedError("Invalid action")

    new_pot_total = state.get_pot_total() + amount
    new_call_amount = (
        max(state.current_bet, player.bet_this_round + amount) - player.bet_this_round
    )

    event = hand.ActionTaken(
        player_root=cmd.player_root,
        action=action,
        amount=amount,
        player_stack=new_stack,
        pot_total=new_pot_total,
        amount_to_call=new_call_amount,
        action_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.poker/")

    return types.EventBook(
        cover=command_book.cover,
        pages=[
            types.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(
                    seconds=int(datetime.now(timezone.utc).timestamp())
                ),
            )
        ],
    )
