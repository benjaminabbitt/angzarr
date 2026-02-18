"""Hand flow process manager gRPC service (OO Pattern).

Orchestrates the flow of poker hands by:
1. Subscribing to table and hand domain events
2. Managing hand process state machines
3. Sending commands to drive hands forward

This example demonstrates the OO pattern using:
- ProcessManager[StateT] base class
- @prepares decorator for destination declaration
- @reacts_to decorator for event handlers
- _create_empty_state() and _apply_event() for state management
"""

import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

import structlog

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client import ProcessManager, prepares, reacts_to
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.process_manager_handler import (
    ProcessManagerHandler,
    run_process_manager_server,
)
from google.protobuf.any_pb2 import Any

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


@dataclass
class PMState:
    """PM's aggregate state (rebuilt from its own events).

    For simplicity in this example, we use a minimal state.
    """

    hand_root: Optional[bytes] = None
    hand_in_progress: bool = False


class HandFlowPM(ProcessManager[PMState]):
    """Hand Flow Process Manager using OO-style decorators.

    This PM orchestrates poker hand flow by:
    - Tracking when hands start and complete
    - Coordinating between table and hand domains
    """

    name = "hand-flow"

    def _create_empty_state(self) -> PMState:
        """Create an empty state instance."""
        return PMState()

    def _apply_event(self, state: PMState, event_any: Any) -> None:
        """Apply a single event to state.

        In this simplified example, we don't persist PM events.
        """
        pass

    @prepares(table.HandStarted)
    def prepare_hand_started(self, event: table.HandStarted) -> list[types.Cover]:
        """Declare the hand destination needed when a hand starts."""
        return [
            types.Cover(
                domain="hand",
                root=types.UUID(value=event.hand_root),
            )
        ]

    @reacts_to(table.HandStarted, input_domain="table")
    def handle_hand_started(
        self,
        event: table.HandStarted,
        destinations: list[types.EventBook],
    ) -> None:
        """Process the HandStarted event.

        Initialize hand process (not persisted in this simplified version).
        The saga-table-hand will send DealCards, so we don't emit commands here.
        """
        return None

    @reacts_to(hand.CardsDealt, input_domain="hand")
    def handle_cards_dealt(
        self,
        event: hand.CardsDealt,
        destinations: list[types.EventBook],
    ) -> None:
        """Process the CardsDealt event.

        Post small blind command. In a real implementation, we'd track state
        to know which blind to post.
        """
        return None

    @reacts_to(hand.BlindPosted, input_domain="hand")
    def handle_blind_posted(
        self,
        event: hand.BlindPosted,
        destinations: list[types.EventBook],
    ) -> None:
        """Process the BlindPosted event.

        In a full implementation, we'd check if both blinds are posted
        and then start the betting round.
        """
        return None

    @reacts_to(hand.ActionTaken, input_domain="hand")
    def handle_action_taken(
        self,
        event: hand.ActionTaken,
        destinations: list[types.EventBook],
    ) -> None:
        """Process the ActionTaken event.

        In a full implementation, we'd check if betting is complete
        and advance to the next phase.
        """
        return None

    @reacts_to(hand.CommunityCardsDealt, input_domain="hand")
    def handle_community_dealt(
        self,
        event: hand.CommunityCardsDealt,
        destinations: list[types.EventBook],
    ) -> None:
        """Process the CommunityCardsDealt event.

        Start new betting round after community cards.
        """
        return None

    @reacts_to(hand.PotAwarded, input_domain="hand")
    def handle_pot_awarded(
        self,
        event: hand.PotAwarded,
        destinations: list[types.EventBook],
    ) -> None:
        """Process the PotAwarded event.

        Hand is complete. Clean up.
        """
        return None


def main():
    """Run the hand flow process manager gRPC service."""
    # OO pattern: pass the PM class directly
    handler = ProcessManagerHandler(HandFlowPM)

    logger.info(
        "hand_flow_pm_starting",
        pattern="OO",
        subscriptions=["table", "hand"],
    )

    run_process_manager_server(
        handler=handler,
        default_port="50492",
        logger=logger,
    )


if __name__ == "__main__":
    main()
