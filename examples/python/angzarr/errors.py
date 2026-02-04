"""Shared error types for angzarr command handlers."""


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""
