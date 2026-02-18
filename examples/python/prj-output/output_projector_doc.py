"""Output projector examples for documentation.

This file contains simplified examples used in the projector documentation,
demonstrating both OO-style and StateRouter patterns.
"""

from typing import Dict

from angzarr_client import Projector, StateRouter
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import hand_pb2 as hand


# docs:start:projector_oo
class OutputProjector(Projector):
    def __init__(self):
        self.player_names: Dict[str, str] = {}

    def handle_player_registered(self, event: player.PlayerRegistered):
        self.player_names[event.player_id] = event.display_name
        print(f"[Player] {event.display_name} registered")

    def handle_funds_deposited(self, event: player.FundsDeposited):
        name = self.player_names.get(event.player_id, event.player_id)
        amount = event.amount.amount if event.amount else 0
        print(f"[Player] {name} deposited ${amount / 100:.2f}")

    def handle_cards_dealt(self, event: hand.CardsDealt):
        for player_cards in event.player_cards:
            name = self.player_names.get(player_cards.player_id, player_cards.player_id)
            cards = format_cards(player_cards.hole_cards)
            print(f"[Hand] {name} dealt {cards}")
# docs:end:projector_oo


def format_cards(cards) -> str:
    return " ".join(f"{c.rank}{c.suit}" for c in cards)


# docs:start:state_router
player_names: Dict[str, str] = {}


def handle_player_registered(event: player.PlayerRegistered):
    player_names[event.player_id] = event.display_name
    print(f"[Player] {event.display_name} registered")


def handle_funds_deposited(event: player.FundsDeposited):
    name = player_names.get(event.player_id, event.player_id)
    print(f"[Player] {name} deposited ${event.amount.amount / 100:.2f}")


def handle_cards_dealt(event: hand.CardsDealt):
    for pc in event.player_cards:
        name = player_names.get(pc.player_id, pc.player_id)
        print(f"[Hand] {name} dealt cards")


router = (
    StateRouter("prj-output")
    .subscribes("player", ["PlayerRegistered", "FundsDeposited"])
    .subscribes("hand", ["CardsDealt", "ActionTaken", "PotAwarded"])
    .on("PlayerRegistered", handle_player_registered)
    .on("FundsDeposited", handle_funds_deposited)
    .on("CardsDealt", handle_cards_dealt)
)
# docs:end:state_router
