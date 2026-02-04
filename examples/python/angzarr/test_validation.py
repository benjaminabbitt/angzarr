"""Tests for validation helpers."""

import pytest

from errors import CommandRejectedError
from validation import (
    require_exists,
    require_non_negative,
    require_not_empty,
    require_not_exists,
    require_positive,
    require_status,
    require_status_not,
)


class TestRequireExists:
    def test_passes_when_non_empty(self):
        require_exists("value", "error")

    def test_fails_when_empty(self):
        with pytest.raises(CommandRejectedError, match="entity not found"):
            require_exists("", "entity not found")


class TestRequireNotExists:
    def test_passes_when_empty(self):
        require_not_exists("", "error")

    def test_fails_when_non_empty(self):
        with pytest.raises(CommandRejectedError, match="already exists"):
            require_not_exists("value", "already exists")


class TestRequirePositive:
    def test_passes(self):
        require_positive(1, "error")
        require_positive(100, "error")

    def test_fails_on_zero(self):
        with pytest.raises(CommandRejectedError, match="must be positive"):
            require_positive(0, "must be positive")

    def test_fails_on_negative(self):
        with pytest.raises(CommandRejectedError):
            require_positive(-1, "error")


class TestRequireNonNegative:
    def test_passes(self):
        require_non_negative(0, "error")
        require_non_negative(1, "error")

    def test_fails(self):
        with pytest.raises(CommandRejectedError):
            require_non_negative(-1, "error")


class TestRequireNotEmpty:
    def test_passes(self):
        require_not_empty([1, 2, 3], "error")

    def test_fails(self):
        with pytest.raises(CommandRejectedError, match="items required"):
            require_not_empty([], "items required")


class TestRequireStatus:
    def test_passes(self):
        require_status("active", "active", "error")

    def test_fails(self):
        with pytest.raises(CommandRejectedError, match="wrong status"):
            require_status("pending", "active", "wrong status")


class TestRequireStatusNot:
    def test_passes(self):
        require_status_not("active", "checked_out", "error")

    def test_fails(self):
        with pytest.raises(CommandRejectedError, match="already checked out"):
            require_status_not("checked_out", "checked_out", "already checked out")
