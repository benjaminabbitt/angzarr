"""Business logic interface for evented-rs."""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Callable, Optional, TypeVar

from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.timestamp_pb2 import Timestamp

from evented.proto import evented_pb2


@dataclass
class CommandContext:
    """Context for command processing."""

    domain: str
    root_id: bytes
    prior_events: list[evented_pb2.EventPage]
    snapshot: Optional[evented_pb2.Snapshot]
    command: ProtoAny
    command_sequence: int
    synchronous: bool


class BusinessLogic(ABC):
    """Base class for business logic implementations.

    Subclass this to implement domain-specific command handling.

    Example:
        class OrdersLogic(BusinessLogic):
            def handle(self, ctx: CommandContext) -> evented_pb2.EventBook:
                if ctx.command.type_url.endswith("CreateOrder"):
                    return self.create_order(ctx)
                elif ctx.command.type_url.endswith("AddItem"):
                    return self.add_item(ctx)
                else:
                    raise ValueError(f"Unknown command: {ctx.command.type_url}")
    """

    @abstractmethod
    def handle(self, ctx: CommandContext) -> evented_pb2.EventBook:
        """Handle a command and return resulting events.

        Args:
            ctx: Command context with prior events and command data

        Returns:
            EventBook containing new events to persist
        """
        pass


# Registry for decorated handlers
_handlers: dict[str, Callable[[CommandContext], evented_pb2.EventBook]] = {}


def business_logic(domain: str):
    """Decorator to register a function as a business logic handler.

    Example:
        @business_logic(domain="orders")
        def handle_orders(ctx: CommandContext) -> EventBook:
            # Handle order commands
            pass
    """

    def decorator(func: Callable[[CommandContext], evented_pb2.EventBook]):
        _handlers[domain] = func
        return func

    return decorator


def handle(domain: str, command_bytes: bytes) -> bytes:
    """Entry point called by Rust PyBusinessLogic.

    This function is called from Rust with serialized protobuf bytes.

    Args:
        domain: The aggregate domain
        command_bytes: Serialized ContextualCommand protobuf

    Returns:
        Serialized EventBook protobuf
    """
    # Deserialize the contextual command
    cmd = evented_pb2.ContextualCommand()
    cmd.ParseFromString(command_bytes)

    # Extract context
    events_book = cmd.events
    command_book = cmd.command

    if not command_book or not command_book.cover:
        raise ValueError("Command must have a cover")

    cover = command_book.cover
    root_id = cover.root.value if cover.root else b""

    # Get prior events
    prior_events = list(events_book.pages) if events_book else []
    snapshot = events_book.snapshot if events_book else None

    # Get command from first page
    if not command_book.pages:
        raise ValueError("Command must have at least one page")

    command_page = command_book.pages[0]

    ctx = CommandContext(
        domain=domain,
        root_id=root_id,
        prior_events=prior_events,
        snapshot=snapshot,
        command=command_page.command,
        command_sequence=command_page.sequence,
        synchronous=command_page.synchronous,
    )

    # Find and call handler
    if domain in _handlers:
        result = _handlers[domain](ctx)
    else:
        raise ValueError(f"No handler registered for domain: {domain}")

    # Serialize and return
    return result.SerializeToString()


def create_event(
    domain: str,
    root_id: bytes,
    sequence: int,
    event_type: str,
    event_data: bytes,
    synchronous: bool = False,
) -> evented_pb2.EventBook:
    """Helper to create an EventBook with a single event.

    Args:
        domain: Aggregate domain
        root_id: Aggregate root UUID bytes
        sequence: Event sequence number
        event_type: Type URL for the event
        event_data: Serialized event payload
        synchronous: Whether this event requires sync processing

    Returns:
        EventBook with the event
    """
    return evented_pb2.EventBook(
        cover=evented_pb2.Cover(
            domain=domain,
            root=evented_pb2.UUID(value=root_id),
        ),
        pages=[
            evented_pb2.EventPage(
                num=sequence,
                Event=ProtoAny(type_url=event_type, value=event_data),
                synchronous=synchronous,
            )
        ],
    )


def create_events(
    domain: str,
    root_id: bytes,
    events: list[tuple[int, str, bytes]],
) -> evented_pb2.EventBook:
    """Helper to create an EventBook with multiple events.

    Args:
        domain: Aggregate domain
        root_id: Aggregate root UUID bytes
        events: List of (sequence, type_url, data) tuples

    Returns:
        EventBook with all events
    """
    pages = [
        evented_pb2.EventPage(
            num=seq,
            Event=ProtoAny(type_url=type_url, value=data),
            synchronous=False,
        )
        for seq, type_url, data in events
    ]

    return evented_pb2.EventBook(
        cover=evented_pb2.Cover(
            domain=domain,
            root=evented_pb2.UUID(value=root_id),
        ),
        pages=pages,
    )
