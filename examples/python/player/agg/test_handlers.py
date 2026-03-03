"""Unit tests for player aggregate handlers.

Tests the guard/validate/compute functions in isolation.
"""

import pytest

from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

from .handlers import deposit_compute, deposit_guard, deposit_validate
from .state import PlayerState


# docs:start:unit_test_deposit
def test_deposit_increases_bankroll():
    """Test that deposit correctly calculates new balance."""
    state = PlayerState()
    state.exists = True
    state.bankroll = 1000

    cmd = player.DepositFunds(
        amount=poker_types.Currency(amount=500, currency_code="CHIPS")
    )

    event = deposit_compute(cmd, state, 500)

    assert event.new_balance.amount == 1500


def test_deposit_rejects_non_existent_player():
    """Test that deposit guard rejects non-existent player."""
    state = PlayerState()
    state.exists = False

    with pytest.raises(CommandRejectedError) as exc_info:
        deposit_guard(state)

    assert "does not exist" in str(exc_info.value)


def test_deposit_rejects_zero_amount():
    """Test that deposit validate rejects zero amount."""
    cmd = player.DepositFunds(
        amount=poker_types.Currency(amount=0, currency_code="CHIPS")
    )

    with pytest.raises(CommandRejectedError) as exc_info:
        deposit_validate(cmd)

    assert "positive" in str(exc_info.value)


# docs:end:unit_test_deposit
