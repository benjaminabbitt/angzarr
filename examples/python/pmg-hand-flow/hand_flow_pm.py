"""Hand Flow Process Manager - OO Pattern.

This PM coordinates the workflow between table and hand domains using
the decorator-based OO pattern with @handles, @prepares, and @output_domain.
"""

from dataclasses import dataclass
from enum import Enum
from typing import Optional

from google.protobuf.any_pb2 import Any

from angzarr_client import run_process_manager_server
from angzarr_client.process_manager import (
    ProcessManager,
    handles,
    output_domain,
    prepares,
)
from angzarr_client.proto.angzarr.types_pb2 import Cover, EventBook, Uuid
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import table_pb2 as table


# docs:start:pm_state_oo
class HandPhase(Enum):
    AWAITING_DEAL = "awaiting_deal"
    DEALING = "dealing"
    BLINDS = "blinds"
    BETTING = "betting"
    COMPLETE = "complete"


@dataclass
class HandFlowState:
    """PM state - tracks workflow progress."""

    hand_id: str = ""
    phase: HandPhase = HandPhase.AWAITING_DEAL
    player_count: int = 0


# docs:end:pm_state_oo


# docs:start:pm_handler_oo
class HandFlowPM(ProcessManager[HandFlowState]):
    """OO-style process manager using decorators."""

    name = "pmg-hand-flow"

    def _create_empty_state(self) -> HandFlowState:
        return HandFlowState()

    def _apply_event(self, state: HandFlowState, event_any: Any) -> None:
        """Apply PM's own events to rebuild state."""
        type_url = event_any.type_url

        if type_url.endswith("HandFlowStarted"):
            # In production, unpack and apply
            pass
        elif type_url.endswith("PhaseTransitioned"):
            pass

    @prepares(table.HandStarted)
    def prepare_hand_started(self, event: table.HandStarted) -> list[Cover]:
        """Declare hand destination needed when hand starts."""
        return [
            Cover(
                domain="hand",
                root=Uuid(value=event.hand_root),
            )
        ]

    @output_domain("hand")
    @handles(table.HandStarted, input_domain="table")
    def on_hand_started(
        self, event: table.HandStarted, destinations: list[EventBook]
    ) -> Optional[hand.DealCards]:
        """Table started a hand -> send DealCards to hand domain."""
        # Update local state
        self.state.hand_id = event.hand_id
        self.state.phase = HandPhase.DEALING
        self.state.player_count = event.player_count

        return hand.DealCards(
            hand_id=event.hand_id,
            player_count=event.player_count,
        )

    @output_domain("hand")
    @handles(hand.CardsDealt, input_domain="hand")
    def on_cards_dealt(self, event: hand.CardsDealt) -> Optional[hand.PostBlinds]:
        """Cards dealt -> post blinds."""
        self.state.phase = HandPhase.BLINDS
        return hand.PostBlinds(hand_id=self.state.hand_id)

    @output_domain("table")
    @handles(hand.HandComplete, input_domain="hand")
    def on_hand_complete(self, event: hand.HandComplete) -> Optional[table.EndHand]:
        """Hand complete -> end hand on table."""
        self.state.phase = HandPhase.COMPLETE
        return table.EndHand(
            hand_id=self.state.hand_id,
            winner_id=event.winner_id,
        )


# docs:end:pm_handler_oo


if __name__ == "__main__":
    run_process_manager_server("pmg-hand-flow", 50391, HandFlowPM)
