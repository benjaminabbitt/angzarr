"""Validation helpers for command handler precondition checks.

Eliminates repeated validation boilerplate across aggregate handlers.
"""

from collections.abc import Sequence
from typing import Any

from .errors import CommandRejectedError


def require_exists(field: str, error_msg: str) -> None:
    """Require that a field is non-empty (entity exists)."""
    if not field:
        raise CommandRejectedError(error_msg)


def require_not_exists(field: str, error_msg: str) -> None:
    """Require that a field is empty (entity does not yet exist)."""
    if field:
        raise CommandRejectedError(error_msg)


def require_positive(value: int, error_msg: str) -> None:
    """Require that a value is greater than zero."""
    if value <= 0:
        raise CommandRejectedError(error_msg)


def require_non_negative(value: int, error_msg: str) -> None:
    """Require that a value is zero or greater."""
    if value < 0:
        raise CommandRejectedError(error_msg)


def require_not_empty(items: Sequence[Any], error_msg: str) -> None:
    """Require that a sequence has at least one element."""
    if not items:
        raise CommandRejectedError(error_msg)


def require_status(actual: str, expected: str, error_msg: str) -> None:
    """Require that the current status matches the expected value."""
    if actual != expected:
        raise CommandRejectedError(error_msg)


def require_status_not(actual: str, forbidden: str, error_msg: str) -> None:
    """Require that the current status is NOT the forbidden value."""
    if actual == forbidden:
        raise CommandRejectedError(error_msg)
