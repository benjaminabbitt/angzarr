"""Hand flow process manager gRPC service.

Orchestrates the flow of poker hands by:
1. Subscribing to table and hand domain events
2. Managing hand process state machines
3. Sending commands to drive hands forward
"""

import sys
from pathlib import Path
from typing import Optional

import structlog

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.process_manager_handler import (
    ProcessManagerHandler,
    run_process_manager_server,
)
from angzarr_client.client import AggregateClient

from hand_process import HandProcessManager

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
                root=cmd_book.cover.root.value.hex()[:8] if cmd_book.cover.root.value else "none",
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
        # when triggered by table events
        destinations = []

        for page in trigger.pages:
            type_url = page.event.type_url
            if "HandStarted" in type_url:
                # Need hand aggregate state
                event = table.HandStarted()
                page.event.Unpack(event)
                destinations.append(types.Cover(
                    root=types.UUID(value=event.hand_root),
                    domain="hand",
                ))

        return destinations

    def handle(
        self,
        trigger: types.EventBook,
        process_state: types.EventBook,
        destinations: list[types.EventBook],
    ) -> tuple[list[types.CommandBook], Optional[types.EventBook]]:
        """Phase 2: Process events and produce commands."""
        commands = []

        for page in trigger.pages:
            event_any = page.event
            type_url = event_any.type_url

            if "HandStarted" in type_url:
                event = table.HandStarted()
                event_any.Unpack(event)
                cmd = self._manager.start_hand(event)
                if cmd:
                    commands.append(cmd)

            elif "CardsDealt" in type_url:
                event = hand.CardsDealt()
                event_any.Unpack(event)
                cmd = self._manager.handle_cards_dealt(event)
                if cmd:
                    commands.append(cmd)

            elif "BlindPosted" in type_url:
                event = hand.BlindPosted()
                event_any.Unpack(event)
                cmd = self._manager.handle_blind_posted(event)
                if cmd:
                    commands.append(cmd)

            elif "ActionTaken" in type_url:
                event = hand.ActionTaken()
                event_any.Unpack(event)
                cmd = self._manager.handle_action_taken(event)
                if cmd:
                    commands.append(cmd)

            elif "BettingRoundComplete" in type_url:
                event = hand.BettingRoundComplete()
                event_any.Unpack(event)
                cmd = self._manager.handle_betting_round_complete(event)
                if cmd:
                    commands.append(cmd)

            elif "CommunityCardsDealt" in type_url:
                event = hand.CommunityCardsDealt()
                event_any.Unpack(event)
                cmd = self._manager.handle_community_cards_dealt(event)
                if cmd:
                    commands.append(cmd)

            elif "ShowdownStarted" in type_url:
                event = hand.ShowdownStarted()
                event_any.Unpack(event)
                cmd = self._manager.handle_showdown_started(event)
                if cmd:
                    commands.append(cmd)

            elif "PotAwarded" in type_url:
                event = hand.PotAwarded()
                event_any.Unpack(event)
                self._manager.handle_pot_awarded(event)

        # No PM-specific events to emit for now
        return commands, None


def main():
    """Run the hand flow process manager gRPC service."""
    pm = HandFlowProcessManager()

    handler = (
        ProcessManagerHandler("hand-flow")
        .listen_to("table", "HandStarted", "HandEnded")
        .listen_to(
            "hand",
            "CardsDealt",
            "BlindPosted",
            "ActionTaken",
            "BettingRoundComplete",
            "CommunityCardsDealt",
            "DrawCompleted",
            "ShowdownStarted",
            "CardsRevealed",
            "CardsMucked",
            "PotAwarded",
            "HandComplete",
        )
        .with_prepare(pm.prepare)
        .with_handle(pm.handle)
    )

    logger.info(
        "hand_flow_pm_starting",
        subscriptions=["table", "hand"],
    )

    run_process_manager_server(
        name="hand-flow",
        default_port="9091",
        handler=handler,
        logger=logger,
    )


if __name__ == "__main__":
    main()
