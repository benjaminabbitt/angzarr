"""Tests for shared error types."""

import pytest

from errors import CommandRejectedError


def test_command_rejected_error_is_exception():
    assert issubclass(CommandRejectedError, Exception)


def test_command_rejected_error_preserves_message():
    err = CommandRejectedError("Cart already exists")
    assert str(err) == "Cart already exists"


def test_command_rejected_error_raises_and_catches():
    with pytest.raises(CommandRejectedError, match="not found"):
        raise CommandRejectedError("not found")
