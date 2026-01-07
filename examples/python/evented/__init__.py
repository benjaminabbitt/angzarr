"""Evented Python SDK for business logic implementation."""

from evented.business_logic import (
    BusinessLogic,
    business_logic,
    CommandContext,
)
from evented.proto import evented_pb2

# Re-export proto types for convenience
ContextualCommand = evented_pb2.ContextualCommand
EventBook = evented_pb2.EventBook
CommandBook = evented_pb2.CommandBook
Cover = evented_pb2.Cover
EventPage = evented_pb2.EventPage
CommandPage = evented_pb2.CommandPage
Snapshot = evented_pb2.Snapshot

__all__ = [
    "BusinessLogic",
    "business_logic",
    "CommandContext",
    "ContextualCommand",
    "EventBook",
    "CommandBook",
    "Cover",
    "EventPage",
    "CommandPage",
    "Snapshot",
]
