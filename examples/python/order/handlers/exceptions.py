"""Command handler exceptions."""


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""
