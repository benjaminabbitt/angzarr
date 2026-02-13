#!/usr/bin/env python3
"""Run a poker game through the angzarr gateway.

Starts angzarr-standalone, then runs a complete poker game with 6 AI players
until one player remains.
"""

import os
import sys
import time
import signal
import subprocess
import argparse
import random
from pathlib import Path
from dataclasses import dataclass
from enum import Enum

# Add paths for imports
root = Path(__file__).parent
sys.path.insert(0, str(root))
sys.path.insert(0, str(root / "angzarr"))
sys.path.insert(0, str(root / "agg-player"))  # Contains proto/poker stubs

import grpc
from client import GatewayClient, derive_root
from angzarr_client.proto.examples import player_pb2, table_pb2, hand_pb2, types_pb2

# Card display
SUIT_SYMBOLS = {
    types_pb2.CLUBS: "♣",
    types_pb2.DIAMONDS: "♦",
    types_pb2.HEARTS: "♥",
    types_pb2.SPADES: "♠",
}

RANK_SYMBOLS = {
    2: "2", 3: "3", 4: "4", 5: "5", 6: "6", 7: "7", 8: "8", 9: "9",
    10: "T", 11: "J", 12: "Q", 13: "K", 14: "A",
}

HAND_NAMES = {
    types_pb2.HIGH_CARD: "High Card",
    types_pb2.PAIR: "Pair",
    types_pb2.TWO_PAIR: "Two Pair",
    types_pb2.THREE_OF_A_KIND: "Three of a Kind",
    types_pb2.STRAIGHT: "Straight",
    types_pb2.FLUSH: "Flush",
    types_pb2.FULL_HOUSE: "Full House",
    types_pb2.FOUR_OF_A_KIND: "Four of a Kind",
    types_pb2.STRAIGHT_FLUSH: "Straight Flush",
    types_pb2.ROYAL_FLUSH: "Royal Flush",
}


class GameVariant(Enum):
    TEXAS_HOLDEM = "holdem"
    FIVE_CARD_DRAW = "draw"


def card_str(card) -> str:
    """Format a card for display."""
    return f"{RANK_SYMBOLS[card.rank]}{SUIT_SYMBOLS[card.suit]}"


def cards_str(cards) -> str:
    """Format multiple cards for display."""
    return "[" + " ".join(card_str(c) for c in cards) + "]"


def chips(amount: int) -> str:
    """Format chip amount."""
    return f"${amount:,}"


@dataclass
class Player:
    """Track player state locally."""
    name: str
    root: bytes
    stack: int
    seat: int
    hole_cards: list = None
    bet: int = 0
    folded: bool = False
    all_in: bool = False
    sequence: int = 0  # Track aggregate sequence

    def __post_init__(self):
        if self.hole_cards is None:
            self.hole_cards = []


class PokerGame:
    """Manages a poker game through the angzarr gateway."""

    def __init__(
        self,
        client: GatewayClient,
        variant: GameVariant = GameVariant.TEXAS_HOLDEM,
        small_blind: int = 5,
        big_blind: int = 10,
        log_file: str = None,
    ):
        self.client = client
        self.variant = variant
        self.small_blind = small_blind
        self.big_blind = big_blind
        self.players: dict[int, Player] = {}
        self.table_root: bytes = None
        self.table_sequence: int = 0
        self.hand_root: bytes = None
        self.hand_sequence: int = 0
        self.hand_num: int = 0
        self.dealer_seat: int = None
        self.pot: int = 0
        self.current_bet: int = 0
        self.community: list = []
        self._log_file = None
        if log_file:
            self._log_file = open(log_file, "w", encoding="utf-8")
            self._log_file.write(f"{'='*60}\n")
            self._log_file.write(f"  ANGZARR POKER - {variant.value.upper()}\n")
            self._log_file.write(f"  Blinds: ${small_blind}/${big_blind}\n")
            self._log_file.write(f"{'='*60}\n\n")
            self._log_file.flush()

    def log(self, msg: str):
        """Print a game message and write to log file."""
        print(msg)
        if self._log_file:
            self._log_file.write(msg + "\n")
            self._log_file.flush()

    def close(self):
        """Close the log file."""
        if self._log_file:
            self._log_file.close()
            self._log_file = None

    def create_table(self, name: str = "Main Table"):
        """Create the poker table."""
        self.table_root = derive_root("table", name.lower().replace(" ", "-"))

        variant_proto = (
            types_pb2.TEXAS_HOLDEM if self.variant == GameVariant.TEXAS_HOLDEM
            else types_pb2.FIVE_CARD_DRAW
        )

        cmd = table_pb2.CreateTable(
            table_name=name,
            game_variant=variant_proto,
            small_blind=self.small_blind,
            big_blind=self.big_blind,
            min_buy_in=self.big_blind * 20,
            max_buy_in=self.big_blind * 100,
            max_players=10,
            action_timeout_seconds=30,
        )

        self.log(f"\n┌─ COMMAND: CreateTable")
        self.log(f"│  table: {name}, variant: {self.variant.value}")
        self.log(f"│  blinds: {chips(self.small_blind)}/{chips(self.big_blind)}")

        resp = self.client.execute("table", self.table_root, cmd, sequence=0)
        self.table_sequence = resp.events.next_sequence

        self.log(f"└─ EVENT: TableCreated")

    def register_player(self, name: str) -> Player:
        """Register a new player."""
        root = derive_root("player", name.lower())

        cmd = player_pb2.RegisterPlayer(
            display_name=name,
            email=f"{name.lower()}@example.com",
            player_type=types_pb2.AI,
        )

        self.log(f"\n┌─ COMMAND: RegisterPlayer")
        self.log(f"│  name: {name}")

        resp = self.client.execute("player", root, cmd, sequence=0)
        sequence = resp.events.next_sequence

        self.log(f"└─ EVENT: PlayerRegistered")

        return Player(name=name, root=root, stack=0, seat=-1, sequence=sequence)

    def deposit_funds(self, player: Player, amount: int):
        """Deposit funds to a player's bankroll."""
        cmd = player_pb2.DepositFunds(
            amount=types_pb2.Currency(amount=amount),
        )

        self.log(f"\n┌─ COMMAND: DepositFunds")
        self.log(f"│  player: {player.name}, amount: {chips(amount)}")

        resp = self.client.execute("player", player.root, cmd, sequence=player.sequence)
        player.sequence = resp.events.next_sequence
        player.stack = amount

        self.log(f"└─ EVENT: FundsDeposited")

    def join_table(self, player: Player, seat: int, buy_in: int):
        """Have a player join the table."""
        # First reserve funds
        cmd = player_pb2.ReserveFunds(
            amount=types_pb2.Currency(amount=buy_in),
            table_root=self.table_root,
        )

        self.log(f"\n┌─ COMMAND: ReserveFunds")
        self.log(f"│  player: {player.name}, amount: {chips(buy_in)}")

        resp = self.client.execute("player", player.root, cmd, sequence=player.sequence)
        player.sequence = resp.events.next_sequence

        self.log(f"└─ EVENT: FundsReserved")

        # Then join table
        cmd = table_pb2.JoinTable(
            player_root=player.root,
            preferred_seat=seat,
            buy_in_amount=buy_in,
        )

        self.log(f"\n┌─ COMMAND: JoinTable")
        self.log(f"│  player: {player.name}, seat: {seat}")

        resp = self.client.execute("table", self.table_root, cmd, sequence=self.table_sequence)
        self.table_sequence = resp.events.next_sequence

        self.log(f"└─ EVENT: PlayerJoined")

        player.seat = seat
        player.stack = buy_in
        self.players[seat] = player

    def add_player(self, name: str, stack: int, seat: int):
        """Convenience: register, deposit, and join in one call."""
        player = self.register_player(name)
        self.deposit_funds(player, stack)
        self.join_table(player, seat, stack)

    def start_hand(self) -> bool:
        """Start a new hand. Returns False if game is over."""
        # Remove eliminated players
        eliminated = [s for s, p in self.players.items() if p.stack <= 0]
        for s in eliminated:
            self.log(f"\n   [{self.players[s].name} eliminated - no chips]")
            del self.players[s]

        if len(self.players) < 2:
            return False

        self.hand_num += 1
        self.pot = 0
        self.current_bet = 0
        self.community = []

        # Reset player state
        for p in self.players.values():
            p.hole_cards = []
            p.bet = 0
            p.folded = False
            p.all_in = False

        # Advance dealer
        seats = sorted(self.players.keys())
        if self.dealer_seat is None:
            self.dealer_seat = seats[0]
        else:
            idx = seats.index(self.dealer_seat) if self.dealer_seat in seats else 0
            self.dealer_seat = seats[(idx + 1) % len(seats)]

        self.log(f"\n{'='*60}")
        self.log(f"HAND #{self.hand_num} - Dealer: {self.players[self.dealer_seat].name}")
        self.log(f"{'='*60}")

        # Create hand root
        self.hand_root = derive_root("hand", f"table-main-{self.hand_num}")
        self.hand_sequence = 0

        # Build player list for deal
        variant_proto = (
            types_pb2.TEXAS_HOLDEM if self.variant == GameVariant.TEXAS_HOLDEM
            else types_pb2.FIVE_CARD_DRAW
        )

        players_in_hand = [
            hand_pb2.PlayerInHand(
                player_root=p.root,
                position=p.seat,
                stack=p.stack,
            )
            for p in sorted(self.players.values(), key=lambda x: x.seat)
        ]

        cmd = hand_pb2.DealCards(
            table_root=self.table_root,
            hand_number=self.hand_num,
            game_variant=variant_proto,
            players=players_in_hand,
            dealer_position=self.dealer_seat,
            small_blind=self.small_blind,
            big_blind=self.big_blind,
            deck_seed=random.randbytes(32),
        )

        self.log(f"\n┌─ COMMAND: DealCards")
        self.log(f"│  hand: #{self.hand_num}, dealer: seat {self.dealer_seat}")

        resp = self.client.execute("hand", self.hand_root, cmd, sequence=0)
        self.hand_sequence = resp.events.next_sequence

        # Parse dealt cards from events
        for page in resp.events.pages:
            if page.event.Is(hand_pb2.CardsDealt.DESCRIPTOR):
                dealt = hand_pb2.CardsDealt()
                page.event.Unpack(dealt)
                for pc in dealt.player_cards:
                    for p in self.players.values():
                        if p.root == pc.player_root:
                            p.hole_cards = list(pc.cards)
                            break

        self.log(f"└─ EVENT: CardsDealt")

        # Show hands to console
        for p in sorted(self.players.values(), key=lambda x: x.seat):
            self.log(f"   {p.name}: {cards_str(p.hole_cards)} ({chips(p.stack)})")

        return True

    def post_blinds(self):
        """Post small and big blinds."""
        seats = sorted(self.players.keys())
        dealer_idx = seats.index(self.dealer_seat)

        if len(seats) == 2:
            sb_seat = self.dealer_seat
            bb_seat = seats[(dealer_idx + 1) % len(seats)]
        else:
            sb_seat = seats[(dealer_idx + 1) % len(seats)]
            bb_seat = seats[(dealer_idx + 2) % len(seats)]

        # Post small blind
        sb_player = self.players[sb_seat]
        sb_amount = min(self.small_blind, sb_player.stack)

        cmd = hand_pb2.PostBlind(
            player_root=sb_player.root,
            blind_type="small",
            amount=sb_amount,
        )

        self.log(f"\n┌─ COMMAND: PostBlind (small)")
        self.log(f"│  {sb_player.name}: {chips(sb_amount)}")

        resp = self.client.execute("hand", self.hand_root, cmd, sequence=self.hand_sequence)
        self.hand_sequence = resp.events.next_sequence

        sb_player.stack -= sb_amount
        sb_player.bet = sb_amount
        self.pot += sb_amount

        self.log(f"└─ EVENT: BlindPosted")

        # Post big blind
        bb_player = self.players[bb_seat]
        bb_amount = min(self.big_blind, bb_player.stack)

        cmd = hand_pb2.PostBlind(
            player_root=bb_player.root,
            blind_type="big",
            amount=bb_amount,
        )

        self.log(f"\n┌─ COMMAND: PostBlind (big)")
        self.log(f"│  {bb_player.name}: {chips(bb_amount)}")

        resp = self.client.execute("hand", self.hand_root, cmd, sequence=self.hand_sequence)
        self.hand_sequence = resp.events.next_sequence

        bb_player.stack -= bb_amount
        bb_player.bet = bb_amount
        self.pot += bb_amount
        self.current_bet = bb_amount

        self.log(f"└─ EVENT: BlindPosted")

    def get_action(self, player: Player) -> tuple[types_pb2.ActionType, int]:
        """Get AI decision for a player. Simplified to mostly call/check/fold."""
        to_call = max(0, self.current_bet - player.bet)

        if to_call == 0:
            # Can check or bet
            if random.random() < 0.2 and player.stack >= self.big_blind and self.current_bet == 0:
                bet_amount = min(self.big_blind * 2, player.stack)
                return types_pb2.BET, bet_amount
            return types_pb2.CHECK, 0
        elif to_call >= player.stack:
            # All-in or fold
            if random.random() < 0.4:
                return types_pb2.CALL, to_call
            return types_pb2.FOLD, 0
        else:
            # Call or fold (no raises to avoid complex validation)
            if random.random() < 0.7:
                return types_pb2.CALL, to_call
            return types_pb2.FOLD, 0

    def betting_round(self, first_to_act_seat: int, preflop: bool = False):
        """Run a betting round."""
        active = [s for s, p in self.players.items() if not p.folded and not p.all_in]
        if len(active) < 2:
            return

        # Reset bets for postflop rounds (preflop keeps blinds)
        if not preflop:
            for p in self.players.values():
                p.bet = 0
            self.current_bet = 0

        seats = sorted(active)
        if first_to_act_seat not in seats:
            first_to_act_seat = seats[0]

        idx = seats.index(first_to_act_seat)
        acted = set()
        last_raiser = None

        while True:
            seat = seats[idx % len(seats)]
            player = self.players[seat]

            if player.folded or player.all_in:
                idx += 1
                continue

            # Check if round is complete
            active_not_allin = [s for s in seats if not self.players[s].folded and not self.players[s].all_in]
            if len(active_not_allin) <= 1:
                break
            if seat in acted and (last_raiser is None or seat == last_raiser):
                break

            action, amount = self.get_action(player)

            cmd = hand_pb2.PlayerAction(
                player_root=player.root,
                action=action,
                amount=amount if action in (types_pb2.CALL, types_pb2.RAISE, types_pb2.BET) else 0,
            )

            action_name = types_pb2.ActionType.Name(action)
            self.log(f"\n┌─ COMMAND: PlayerAction")
            self.log(f"│  {player.name}: {action_name}" + (f" {chips(amount)}" if amount else ""))

            resp = self.client.execute("hand", self.hand_root, cmd, sequence=self.hand_sequence)
            self.hand_sequence = resp.events.next_sequence

            self.log(f"└─ EVENT: ActionTaken")

            # Update local state
            if action == types_pb2.FOLD:
                player.folded = True
            elif action == types_pb2.CALL:
                call_amount = min(self.current_bet - player.bet, player.stack)
                player.stack -= call_amount
                player.bet += call_amount
                self.pot += call_amount
                if player.stack == 0:
                    player.all_in = True
            elif action in (types_pb2.BET, types_pb2.RAISE):
                bet_amount = amount - player.bet
                player.stack -= bet_amount
                player.bet = amount
                self.pot += bet_amount
                self.current_bet = amount
                last_raiser = seat
                if player.stack == 0:
                    player.all_in = True

            acted.add(seat)
            idx += 1

            # Remove folded from active
            seats = [s for s in sorted(self.players.keys()) if not self.players[s].folded and not self.players[s].all_in]
            if len(seats) < 2:
                break

    def deal_community(self, count: int, phase_name: str):
        """Deal community cards."""
        cmd = hand_pb2.DealCommunityCards(count=count)

        self.log(f"\n┌─ COMMAND: DealCommunityCards ({phase_name})")

        resp = self.client.execute("hand", self.hand_root, cmd, sequence=self.hand_sequence)
        self.hand_sequence = resp.events.next_sequence

        # Parse dealt cards
        for page in resp.events.pages:
            if page.event.Is(hand_pb2.CommunityCardsDealt.DESCRIPTOR):
                dealt = hand_pb2.CommunityCardsDealt()
                page.event.Unpack(dealt)
                self.community = list(dealt.all_community_cards)

        self.log(f"└─ EVENT: CommunityCardsDealt")
        self.log(f"   Board: {cards_str(self.community)}")

    def showdown(self):
        """Determine winner and award pot."""
        active = [p for p in self.players.values() if not p.folded]

        if len(active) == 1:
            winner = active[0]
            self.log(f"\n   {winner.name} wins {chips(self.pot)} (others folded)")
            winner.stack += self.pot
        else:
            # For simplicity, pick random winner among active
            # In real implementation, evaluate hands
            winner = random.choice(active)
            self.log(f"\n   {winner.name} wins {chips(self.pot)}")
            winner.stack += self.pot

        # Award pot command
        cmd = hand_pb2.AwardPot(
            awards=[
                hand_pb2.PotAward(
                    player_root=winner.root,
                    amount=self.pot,
                    pot_type="main",
                )
            ]
        )

        self.log(f"\n┌─ COMMAND: AwardPot")
        self.log(f"│  {winner.name}: {chips(self.pot)}")

        resp = self.client.execute("hand", self.hand_root, cmd, sequence=self.hand_sequence)
        self.hand_sequence = resp.events.next_sequence

        self.log(f"└─ EVENT: PotAwarded")

    def play_hand(self):
        """Play a complete hand."""
        if not self.start_hand():
            return False

        self.post_blinds()

        # Determine first to act
        seats = sorted(self.players.keys())
        dealer_idx = seats.index(self.dealer_seat)

        if len(seats) == 2:
            first_preflop = self.dealer_seat  # Heads up: dealer acts first preflop
        else:
            first_preflop = seats[(dealer_idx + 3) % len(seats)]  # UTG

        # Preflop betting
        self.log(f"\n--- PREFLOP ---")
        self.betting_round(first_preflop, preflop=True)

        # Check if hand is over (all but one folded)
        active = [p for p in self.players.values() if not p.folded]
        if len(active) == 1:
            self.showdown()
            return True

        if self.variant == GameVariant.TEXAS_HOLDEM:
            # Flop
            self.deal_community(3, "FLOP")
            self.log(f"\n--- FLOP ---")
            first_postflop = seats[(dealer_idx + 1) % len(seats)]
            self.betting_round(first_postflop)

            active = [p for p in self.players.values() if not p.folded]
            if len(active) == 1:
                self.showdown()
                return True

            # Turn
            self.deal_community(1, "TURN")
            self.log(f"\n--- TURN ---")
            self.betting_round(first_postflop)

            active = [p for p in self.players.values() if not p.folded]
            if len(active) == 1:
                self.showdown()
                return True

            # River
            self.deal_community(1, "RIVER")
            self.log(f"\n--- RIVER ---")
            self.betting_round(first_postflop)

        self.showdown()
        return True

    def show_standings(self):
        """Show current chip counts."""
        self.log(f"\n--- STANDINGS ---")
        for p in sorted(self.players.values(), key=lambda x: -x.stack):
            self.log(f"   {p.name}: {chips(p.stack)}")

    def play_tournament(self, max_hands: int = 100):
        """Play until one player remains or max hands reached."""
        hands_played = 0
        while len(self.players) > 1 and hands_played < max_hands:
            self.play_hand()
            self.show_standings()
            hands_played += 1
            time.sleep(0.1)  # Brief pause between hands

        if len(self.players) == 1:
            winner = list(self.players.values())[0]
            self.log(f"\n{'='*60}")
            self.log(f"TOURNAMENT WINNER: {winner.name} with {chips(winner.stack)}")
            self.log(f"{'='*60}")
        else:
            self.log(f"\n{'='*60}")
            self.log(f"Tournament ended after {max_hands} hands")
            self.log(f"{'='*60}")


def start_standalone() -> subprocess.Popen:
    """Start angzarr-standalone in the background."""
    # Kill any existing processes from previous runs
    subprocess.run(["pkill", "-9", "-f", "angzarr-standalone"], capture_output=True)
    subprocess.run(["pkill", "-9", "-f", "agg-player"], capture_output=True)
    subprocess.run(["pkill", "-9", "-f", "agg-table"], capture_output=True)
    subprocess.run(["pkill", "-9", "-f", "agg-hand"], capture_output=True)
    time.sleep(0.5)  # Let processes fully terminate

    # Clean up old sockets
    for sock in Path("tmp").glob("*.sock"):
        sock.unlink()

    # Clean up SQLite databases for fresh start
    for db_file in Path("data").glob("*.db*"):
        db_file.unlink()

    env = os.environ.copy()
    env["ANGZARR_CONFIG"] = "standalone.yaml"

    proc = subprocess.Popen(
        ["./bin/angzarr-standalone"],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )

    # Wait for gateway to be ready (5s for aggregates + projector)
    print("Starting angzarr-standalone...")
    time.sleep(5)

    return proc


def main():
    parser = argparse.ArgumentParser(description="Run a poker game")
    parser.add_argument(
        "--variant",
        choices=["holdem", "draw"],
        default="holdem",
        help="Game variant (default: holdem)",
    )
    parser.add_argument(
        "--players",
        type=int,
        default=6,
        help="Number of players (default: 6)",
    )
    parser.add_argument(
        "--stack",
        type=int,
        default=1000,
        help="Starting stack per player (default: 1000)",
    )
    parser.add_argument(
        "--max-hands",
        type=int,
        default=100,
        help="Maximum hands to play (default: 100)",
    )
    parser.add_argument(
        "--no-standalone",
        action="store_true",
        help="Don't start standalone (assume it's already running)",
    )
    args = parser.parse_args()

    proc = None
    if not args.no_standalone:
        proc = start_standalone()

    try:
        variant = GameVariant.TEXAS_HOLDEM if args.variant == "holdem" else GameVariant.FIVE_CARD_DRAW

        with GatewayClient("localhost:9084") as client:
            game = PokerGame(client, variant=variant)

            # Setup
            game.create_table("Main Table")

            names = ["Alice", "Bob", "Carol", "Dave", "Eve", "Frank", "Grace", "Hank"]
            for i in range(min(args.players, len(names))):
                game.add_player(names[i], args.stack, i)

            # Play
            game.play_tournament(max_hands=args.max_hands)

    except KeyboardInterrupt:
        print("\nGame interrupted")
    except grpc.RpcError as e:
        print(f"\nRPC Error: {e.code()}: {e.details()}")
    finally:
        if proc:
            print("\nShutting down standalone...")
            proc.terminate()
            proc.wait(timeout=5)


if __name__ == "__main__":
    main()
