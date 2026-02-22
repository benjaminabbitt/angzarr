"""Hand Flow Process Manager - orchestrates poker hand phases across domains.

This PM coordinates the workflow between table and hand domains,
tracking phase transitions and dispatching commands as the hand progresses.
"""

from dataclasses import dataclass
from enum import Enum

from angzarr_client import ProcessManager
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import table_pb2 as table


# docs:start:pm_state
class HandPhase(Enum):
    AWAITING_DEAL = "awaiting_deal"
    DEALING = "dealing"
    BLINDS = "blinds"
    BETTING = "betting"
    COMPLETE = "complete"


@dataclass
class HandFlowState:
    hand_id: str = ""
    phase: HandPhase = HandPhase.AWAITING_DEAL
    player_count: int = 0


# docs:end:pm_state


# docs:start:pm_handler
class HandFlowPM(ProcessManager):
    def handle_hand_started(self, event: table.HandStarted, state: HandFlowState):
        # Transition: AWAITING_DEAL -> DEALING
        state.hand_id = event.hand_id
        state.phase = HandPhase.DEALING
        state.player_count = event.player_count

        # Emit command to hand domain
        return [
            hand.DealCards(
                hand_id=event.hand_id,
                player_count=event.player_count,
            )
        ]

    def handle_cards_dealt(self, event: hand.CardsDealt, state: HandFlowState):
        # Transition: DEALING -> BLINDS
        state.phase = HandPhase.BLINDS
        return [hand.PostBlinds(hand_id=state.hand_id)]

    def handle_hand_complete(self, event: hand.HandComplete, state: HandFlowState):
        # Transition: * -> COMPLETE
        state.phase = HandPhase.COMPLETE

        # Signal table domain
        return [
            table.EndHand(
                hand_id=state.hand_id,
                winner_id=event.winner_id,
            )
        ]


# docs:end:pm_handler
