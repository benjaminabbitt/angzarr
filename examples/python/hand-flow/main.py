"""Hand flow process manager gRPC service.

Orchestrates the flow of poker hands by:
1. Subscribing to table and hand domain events
2. Managing hand process state machines
3. Sending commands to drive hands forward
"""

import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Optional

import structlog
from google.protobuf.message import Message

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from hand_process import HandProcessManager

from angzarr_client.client import AggregateClient
from angzarr_client.helpers import destination_map, next_sequence
from angzarr_client.process_manager_handler import (
    ProcessManagerHandler,
    run_process_manager_server,
)
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import table_pb2 as table


@dataclass
class EventHandler:
    """Event handler registration for dispatch table."""

    proto_class: type[Message]
    handler: Callable
    returns_command: bool = True


structlog.configure(
    processors=[
        structlog.stdlib.add_log_level,
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.JSONRenderer(),
    ],
    wrapper_class=structlog.make_filtering_bound_logger(0),
    context_class=dict,
    logger_factory=structlog.PrintLoggerFactory(),
)

logger = structlog.get_logger()


class HandFlowProcessManager:
    """Wrapper that integrates HandProcessManager with gRPC handler."""

    def __init__(self, client: Optional[AggregateClient] = None):
        self._client = client
        self._manager = HandProcessManager(
            command_sender=self._send_command,
            timeout_handler=self._handle_timeout,
        )
        # Event dispatch table - maps type_url suffix to handler info
        # HandStarted is handled specially (different domain, needs table_root)
        self._event_handlers: dict[str, EventHandler] = {
            "CardsDealt": EventHandler(
                proto_class=hand.CardsDealt,
                handler=self._manager.handle_cards_dealt,
            ),
            "BlindPosted": EventHandler(
                proto_class=hand.BlindPosted,
                handler=self._manager.handle_blind_posted,
            ),
            "ActionTaken": EventHandler(
                proto_class=hand.ActionTaken,
                handler=self._manager.handle_action_taken,
            ),
            "BettingRoundComplete": EventHandler(
                proto_class=hand.BettingRoundComplete,
                handler=self._manager.handle_betting_round_complete,
            ),
            "CommunityCardsDealt": EventHandler(
                proto_class=hand.CommunityCardsDealt,
                handler=self._manager.handle_community_cards_dealt,
            ),
            "ShowdownStarted": EventHandler(
                proto_class=hand.ShowdownStarted,
                handler=self._manager.handle_showdown_started,
            ),
            "PotAwarded": EventHandler(
                proto_class=hand.PotAwarded,
                handler=self._manager.handle_pot_awarded,
                returns_command=False,
            ),
        }

    def _send_command(self, cmd_book: types.CommandBook) -> None:
        """Send a command via gRPC client."""
        if self._client:
            try:
                self._client.handle(cmd_book)
            except Exception as e:
                logger.error("command_send_failed", error=str(e))
        else:
            logger.info(
                "command_would_send",
                domain=cmd_book.cover.domain,
                root=(
                    cmd_book.cover.root.value.hex()[:8]
                    if cmd_book.cover.root.value
                    else "none"
                ),
            )

    def _handle_timeout(self, hand_id: bytes, player_position: int) -> None:
        """Handle action timeout."""
        logger.info(
            "action_timeout",
            hand_id=hand_id.hex()[:8],
            position=player_position,
        )

    def prepare(
        self,
        trigger: types.EventBook,
        process_state: types.EventBook,
    ) -> list[types.Cover]:
        """Phase 1: Declare additional destinations needed."""
        # Hand flow PM needs to fetch hand aggregate state
        # when triggered by table or hand events
        destinations = []

        # Check trigger domain - if it's hand domain, we need its state
        trigger_domain = trigger.cover.domain if trigger.cover else ""

        for page in trigger.pages:
            type_url = page.event.type_url
            if "HandStarted" in type_url:
                # Table event - extract hand_root from event payload
                event = table.HandStarted()
                page.event.Unpack(event)
                destinations.append(
                    types.Cover(
                        root=types.UUID(value=event.hand_root),
                        domain="hand",
                    )
                )
            elif trigger_domain == "hand":
                # Hand domain events - use trigger's root directly
                # Need state for sequence numbers on subsequent commands
                if trigger.cover and trigger.cover.root:
                    destinations.append(
                        types.Cover(
                            root=trigger.cover.root,
                            domain="hand",
                        )
                    )
                    break  # Only need one destination per hand

        return destinations

    def _dispatch_event(
        self,
        type_url: str,
        event_any,
        correlation_id: str,
    ) -> Optional[types.CommandBook]:
        """Dispatch event through handler registry."""
        for suffix, handler_info in self._event_handlers.items():
            if suffix in type_url:
                event = handler_info.proto_class()
                event_any.Unpack(event)
                result = handler_info.handler(correlation_id, event)
                return result if handler_info.returns_command else None
        return None

    def handle(
        self,
        trigger: types.EventBook,
        process_state: types.EventBook,
        destinations: list[types.EventBook],
    ) -> tuple[list[types.CommandBook], Optional[types.EventBook]]:
        """Phase 2: Process events and produce commands."""
        commands = []

        # Get correlation_id from trigger - used as process key
        correlation_id = trigger.cover.correlation_id if trigger.cover else ""
        table_root = (
            trigger.cover.root.value if trigger.cover and trigger.cover.root else b""
        )

        # Build destination map for sequence lookup
        dest_map = destination_map(destinations)

        for page in trigger.pages:
            event_any = page.event
            type_url = event_any.type_url

            # HandStarted is special - from table domain, initializes process
            if "HandStarted" in type_url:
                event = table.HandStarted()
                event_any.Unpack(event)
                self._manager.start_hand(event, table_root, correlation_id)
            else:
                # Dispatch through handler registry
                cmd = self._dispatch_event(type_url, event_any, correlation_id)
                if cmd:
                    commands.append(cmd)

        # Set sequences and correlation_id on commands from destination state
        for cmd in commands:
            if cmd.cover:
                cmd.cover.correlation_id = correlation_id
                if cmd.cover.root and cmd.cover.root.value:
                    root_hex = cmd.cover.root.value.hex()
                    dest = dest_map.get(root_hex)
                    seq = next_sequence(dest)
                    for cmd_page in cmd.pages:
                        cmd_page.header.CopyFrom(types.PageHeader(sequence=seq))

        # No PM-specific events to emit for now
        return commands, None


def main():
    """Run the hand flow process manager gRPC service."""
    pm = HandFlowProcessManager()

    # Subscriptions configured via ANGZARR__MESSAGING__AMQP__DOMAIN env var
    # The coordinator routes: table.HandStarted, table.HandEnded, hand.*
    handler = (
        ProcessManagerHandler("hand-flow")
        .with_prepare(pm.prepare)
        .with_handle(pm.handle)
    )

    logger.info(
        "hand_flow_pm_starting",
        subscriptions=["table", "hand"],
    )

    run_process_manager_server(
        handler=handler,
        default_port="50391",
        logger=logger,
    )


if __name__ == "__main__":
    main()
