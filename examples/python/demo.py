#!/usr/bin/env python3
"""Demo script showing full event-sourced poker flow with all commands/events."""

import sys
import argparse
from pathlib import Path
from dataclasses import dataclass
from enum import Enum

# Add paths for proto imports
sys.path.insert(0, str(Path(__file__).parent / "agg-player"))
sys.path.insert(0, str(Path(__file__).parent / "agg-hand"))

from angzarr_client.proto.examples import poker_types_pb2 as poker_types
from handlers.game_rules import get_game_rules, FiveCardDrawRules
from handlers.ai import PokerAI
from handlers.betting import BettingRound, DrawRound, PlayerState

# Card symbols
SUIT_SYMBOLS = {
    poker_types.CLUBS: "♣",
    poker_types.DIAMONDS: "♦",
    poker_types.HEARTS: "♥",
    poker_types.SPADES: "♠",
}

RANK_SYMBOLS = {
    2: "2", 3: "3", 4: "4", 5: "5", 6: "6", 7: "7", 8: "8", 9: "9",
    10: "T", 11: "J", 12: "Q", 13: "K", 14: "A",
}

HAND_NAMES = {
    poker_types.HIGH_CARD: "High Card",
    poker_types.PAIR: "Pair",
    poker_types.TWO_PAIR: "Two Pair",
    poker_types.THREE_OF_A_KIND: "Three of a Kind",
    poker_types.STRAIGHT: "Straight",
    poker_types.FLUSH: "Flush",
    poker_types.FULL_HOUSE: "Full House",
    poker_types.FOUR_OF_A_KIND: "Four of a Kind",
    poker_types.STRAIGHT_FLUSH: "Straight Flush",
    poker_types.ROYAL_FLUSH: "Royal Flush",
}


class GameVariant(Enum):
    TEXAS_HOLDEM = "holdem"
    FIVE_CARD_DRAW = "draw"


VARIANT_NAMES = {
    GameVariant.TEXAS_HOLDEM: "Texas Hold'em",
    GameVariant.FIVE_CARD_DRAW: "Five Card Draw",
}

VARIANT_PROTO = {
    GameVariant.TEXAS_HOLDEM: poker_types.TEXAS_HOLDEM,
    GameVariant.FIVE_CARD_DRAW: poker_types.FIVE_CARD_DRAW,
}


def card_str(suit, rank):
    return f"{RANK_SYMBOLS[rank]}{SUIT_SYMBOLS[suit]}"


def cards_str(cards):
    return "[" + " ".join(card_str(s, r) for s, r in cards) + "]"


def chips(amount):
    return f"${amount:,}"


class Domain(Enum):
    USER = "User"
    TABLE = "Table"
    HAND = "Hand"
    PLAYER = "Player"
    TABLE_SYNC_SAGA = "TableSyncSaga"
    HAND_RESULTS_SAGA = "HandResultsSaga"
    PROCESS_MANAGER = "ProcessManager"
    OUTPUT_SAGA = "OutputSaga"


@dataclass
class Player:
    name: str
    stack: int
    seat: int
    hole_cards: list = None
    bet: int = 0
    folded: bool = False
    all_in: bool = False

    def __post_init__(self):
        if self.hole_cards is None:
            self.hole_cards = []


class EventSourcedPokerGame:
    """Poker game that logs all commands and events with domains."""

    def __init__(self, variant: GameVariant = GameVariant.TEXAS_HOLDEM,
                 small_blind=5, big_blind=10, output=print):
        self.variant = variant
        self.players = {}
        self.pot = 0
        self.current_bet = 0
        self.community = []
        self.dealer_seat = None
        self.small_blind = small_blind
        self.big_blind = big_blind
        self.log = output
        self.rules = get_game_rules(VARIANT_PROTO[variant])
        self.ai = PokerAI(self.rules)
        self.deck = []
        self.hand_num = 0

    def _log(self, msg):
        self.log(msg)

    def _command(self, source: Domain, target: Domain, cmd: str, params: str = ""):
        self._log(f"")
        self._log(f"┌─ COMMAND: {cmd}")
        self._log(f"│  {source.value} → {target.value}")
        if params:
            for line in params.split("\n"):
                self._log(f"│  {line}")

    def _event(self, source: Domain, event: str, data: str = "", targets: list = None):
        self._log(f"│")
        self._log(f"└─ EVENT: {event}")
        self._log(f"   ← {source.value}")
        if data:
            for line in data.split("\n"):
                self._log(f"   {line}")
        if targets:
            self._log(f"   ──► received by: {', '.join(t.value for t in targets)}")

    def _saga_reaction(self, saga: Domain, received: str, action: str):
        self._log(f"")
        self._log(f"   ┌─ {saga.value} reacts to {received}")
        self._log(f"   └─► {action}")

    def _pm_reaction(self, received: str, action: str):
        self._log(f"")
        self._log(f"   ┌─ ProcessManager reacts to {received}")
        self._log(f"   └─► {action}")

    # ─────────────────────────────────────────────────────────────
    # SETUP PHASE
    # ─────────────────────────────────────────────────────────────

    def create_table(self, name: str):
        variant_name = VARIANT_NAMES[self.variant]
        self._command(Domain.USER, Domain.TABLE, "CreateTable",
            f"table_name: {name}\n"
            f"game_variant: {variant_name}\n"
            f"small_blind: {self.small_blind}\n"
            f"big_blind: {self.big_blind}")

        self._event(Domain.TABLE, "TableCreated",
            f"table_root: table-{name.lower().replace(' ', '-')}",
            [Domain.OUTPUT_SAGA])

    def register_player(self, name: str, player_type: str = "AI"):
        player_root = f"player-{name.lower()}"

        self._command(Domain.USER, Domain.PLAYER, "RegisterPlayer",
            f"display_name: {name}\n"
            f"player_type: {player_type}")

        self._event(Domain.PLAYER, "PlayerRegistered",
            f"player_root: {player_root}\n"
            f"display_name: {name}",
            [Domain.OUTPUT_SAGA])

        return player_root

    def deposit_funds(self, player_root: str, amount: int):
        name = player_root.replace("player-", "").title()

        self._command(Domain.USER, Domain.PLAYER, "DepositFunds",
            f"player_root: {player_root}\n"
            f"amount: {amount}")

        self._event(Domain.PLAYER, "FundsDeposited",
            f"player: {name}\n"
            f"amount: {amount}\n"
            f"new_balance: {amount}",
            [Domain.OUTPUT_SAGA])

    def reserve_and_join(self, player_root: str, seat: int, buy_in: int):
        name = player_root.replace("player-", "").title()

        self._command(Domain.USER, Domain.PLAYER, "ReserveFunds",
            f"player_root: {player_root}\n"
            f"amount: {buy_in}\n"
            f"table_root: table-main")

        self._event(Domain.PLAYER, "FundsReserved",
            f"player: {name}\n"
            f"amount: {buy_in}\n"
            f"new_reserved: {buy_in}\n"
            f"new_available: 0",
            [Domain.OUTPUT_SAGA])

        self._command(Domain.USER, Domain.TABLE, "JoinTable",
            f"player_root: {player_root}\n"
            f"seat: {seat}\n"
            f"buy_in: {buy_in}")

        self._event(Domain.TABLE, "PlayerJoined",
            f"player: {name}\n"
            f"seat: {seat}\n"
            f"stack: {buy_in}",
            [Domain.OUTPUT_SAGA])

        self.players[seat] = Player(name, buy_in, seat)

    def add_player(self, name: str, stack: int, seat: int):
        """Convenience method that does full registration flow."""
        player_root = self.register_player(name)
        self.deposit_funds(player_root, stack)
        self.reserve_and_join(player_root, seat, stack)

    # ─────────────────────────────────────────────────────────────
    # HAND START
    # ─────────────────────────────────────────────────────────────

    def start_hand(self) -> bool:
        self.hand_num += 1
        self.pot = 0
        self.current_bet = 0
        self.community = []
        self.deck = self.rules.create_deck()

        # Remove eliminated players
        eliminated = [s for s, p in self.players.items() if p.stack <= 0]
        for s in eliminated:
            self._log(f"\n   [{self.players[s].name} eliminated - no chips]")
            del self.players[s]

        if len(self.players) < 2:
            return False

        # Reset player state
        for p in self.players.values():
            p.hole_cards = []
            p.bet = 0
            p.folded = False
            p.all_in = False

        # Advance dealer
        seats = sorted(self.players.keys())
        if self.dealer_seat is None or self.dealer_seat not in seats:
            self.dealer_seat = seats[0]
        else:
            idx = seats.index(self.dealer_seat)
            self.dealer_seat = seats[(idx + 1) % len(seats)]

        dealer_name = self.players[self.dealer_seat].name

        # Determine blinds
        dealer_idx = seats.index(self.dealer_seat)
        if len(seats) == 2:
            sb_seat = self.dealer_seat
            bb_seat = seats[(dealer_idx + 1) % len(seats)]
        else:
            sb_seat = seats[(dealer_idx + 1) % len(seats)]
            bb_seat = seats[(dealer_idx + 2) % len(seats)]

        variant_name = VARIANT_NAMES[self.variant]
        self._log(f"\n{'='*70}")
        self._log(f"  HAND #{self.hand_num} - {variant_name}")
        self._log(f"{'='*70}")

        self._command(Domain.USER, Domain.TABLE, "StartHand", "")

        players_info = "\n".join(
            f"  seat {s}: {p.name} ({chips(p.stack)})"
            for s, p in sorted(self.players.items())
        )
        self._event(Domain.TABLE, "HandStarted",
            f"hand_number: {self.hand_num}\n"
            f"game_variant: {variant_name}\n"
            f"dealer: {dealer_name} (seat {self.dealer_seat})\n"
            f"small_blind: {self.players[sb_seat].name} (seat {sb_seat})\n"
            f"big_blind: {self.players[bb_seat].name} (seat {bb_seat})\n"
            f"players:\n{players_info}",
            [Domain.OUTPUT_SAGA, Domain.TABLE_SYNC_SAGA, Domain.PROCESS_MANAGER])

        self._saga_reaction(Domain.TABLE_SYNC_SAGA, "HandStarted",
            "dispatches DealCards to Hand aggregate")

        return True

    def deal_cards(self):
        """Deal hole cards - triggered by TableSyncSaga."""
        variant_name = VARIANT_NAMES[self.variant]
        num_cards = self.rules.hole_card_count

        self._command(Domain.TABLE_SYNC_SAGA, Domain.HAND, "DealCards",
            f"hand_number: {self.hand_num}\n"
            f"game_variant: {variant_name}\n"
            f"cards_per_player: {num_cards}\n"
            f"deck_seed: <random>")

        # Deal cards based on variant
        cards_info = []
        for p in self.seated_order():
            p.hole_cards = [self.deck.pop() for _ in range(num_cards)]
            cards_info.append(f"  {p.name}: {cards_str(p.hole_cards)}")

        self._event(Domain.HAND, "CardsDealt",
            f"player_cards:\n" + "\n".join(cards_info),
            [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

        self._pm_reaction("CardsDealt", "transitions to POSTING_BLINDS, dispatches PostBlind (small)")

    def post_blinds(self) -> int:
        """Post blinds - triggered by ProcessManager."""
        seats = sorted(self.players.keys())
        dealer_idx = seats.index(self.dealer_seat)

        if len(seats) == 2:
            sb_seat = self.dealer_seat
            bb_seat = seats[(dealer_idx + 1) % len(seats)]
        else:
            sb_seat = seats[(dealer_idx + 1) % len(seats)]
            bb_seat = seats[(dealer_idx + 2) % len(seats)]

        self._post_blind(sb_seat, "SMALL_BLIND", self.small_blind)
        self._pm_reaction("BlindPosted (small)", "dispatches PostBlind (big)")

        self._post_blind(bb_seat, "BIG_BLIND", self.big_blind)
        self._pm_reaction("BlindPosted (big)",
            "transitions to BETTING (preflop), dispatches RequestAction")

        return bb_seat

    def _post_blind(self, seat: int, blind_type: str, amount: int):
        p = self.players[seat]
        actual = min(amount, p.stack)

        self._command(Domain.PROCESS_MANAGER, Domain.HAND, "PostBlind",
            f"player: {p.name}\n"
            f"blind_type: {blind_type}\n"
            f"amount: {amount}")

        p.stack -= actual
        p.bet = actual
        self.pot += actual
        if actual < amount:
            p.all_in = True
        if blind_type == "BIG_BLIND":
            self.current_bet = actual

        self._event(Domain.HAND, "BlindPosted",
            f"player: {p.name}\n"
            f"blind_type: {blind_type}\n"
            f"amount: {actual}\n"
            f"player_stack: {p.stack}\n"
            f"pot: {self.pot}",
            [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

    # ─────────────────────────────────────────────────────────────
    # BETTING ROUNDS (uses angzarr BettingRound)
    # ─────────────────────────────────────────────────────────────

    def betting_round(self, first_to_act_seat: int, phase_name: str) -> bool:
        """Run a betting round using BettingRound from angzarr."""
        if phase_name != "PREFLOP":
            for p in self.players.values():
                p.bet = 0
            self.current_bet = 0

        # Build player states for BettingRound
        player_states = [
            PlayerState(
                seat=p.seat, stack=p.stack, bet=p.bet,
                folded=p.folded, all_in=p.all_in
            )
            for p in self.players.values()
        ]

        betting = BettingRound(
            players=player_states,
            first_to_act_seat=first_to_act_seat,
            current_bet=self.current_bet,
            pot=self.pot,
        )

        while not betting.is_complete():
            seat = betting.get_next_to_act()
            if seat is None:
                break

            p = self.players[seat]
            to_call = betting.get_to_call(seat)

            # Log request
            self._request_action(p, to_call, phase_name)

            # Get AI decision
            is_holdem_preflop = (
                self.variant == GameVariant.TEXAS_HOLDEM and not self.community
            )
            decision = self.ai.decide_action(
                hole_cards=p.hole_cards,
                community_cards=self.community,
                to_call=to_call,
                pot=betting.pot,
                current_bet=betting.current_bet,
                stack=p.stack,
                big_blind=self.big_blind,
                is_holdem_preflop=is_holdem_preflop,
            )

            self._log(f"   │  Strength: {decision.reasoning}")
            self._log(f"   └─►")

            # Process action through BettingRound
            result = betting.process_action(seat, decision.action, decision.amount)

            # Sync local state
            ps = betting.players[seat]
            p.stack = ps.stack
            p.bet = ps.bet
            p.folded = ps.folded
            p.all_in = ps.all_in
            self.pot = betting.pot
            self.current_bet = betting.current_bet

            # Log action event
            self._log_action(p, decision.action, decision.amount, result)

            if result.raised:
                self._pm_reaction("ActionTaken (raise/bet)",
                    "reopens betting, dispatches RequestAction to next player")
            elif not betting.is_complete():
                self._pm_reaction("ActionTaken", "dispatches RequestAction to next player")

            if not result.hand_continues:
                return False

        self._event(Domain.HAND, "BettingRoundComplete",
            f"phase: {phase_name}\n"
            f"pot: {self.pot}",
            [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

        return len([x for x in self.players.values() if not x.folded]) > 1

    def _request_action(self, p: Player, to_call: int, phase: str):
        self._command(Domain.PROCESS_MANAGER, Domain.PLAYER, "RequestAction",
            f"player: {p.name}\n"
            f"hole_cards: {cards_str(p.hole_cards)}\n"
            f"community: {cards_str(self.community) if self.community else '[]'}\n"
            f"to_call: {to_call}\n"
            f"pot: {self.pot}\n"
            f"phase: {phase}")

        self._event(Domain.PLAYER, "ActionRequested",
            f"player: {p.name}\n"
            f"player_type: AI\n"
            f"deadline: +30s",
            [Domain.OUTPUT_SAGA])

        self._log(f"")
        self._log(f"   ┌─ AI Decision for {p.name}")
        self._log(f"   │  Hand: {cards_str(p.hole_cards)}" +
                  (f" + {cards_str(self.community)}" if self.community else ""))

    def _log_action(self, p: Player, action: str, amount: int, result):
        """Log the action command and event."""
        if action == "fold":
            self._command(Domain.PLAYER, Domain.HAND, "PlayerAction",
                f"player: {p.name}\n"
                f"action: FOLD")
            self._event(Domain.HAND, "ActionTaken",
                f"player: {p.name}\n"
                f"action: FOLD\n"
                f"pot: {self.pot}",
                [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

        elif action == "check":
            self._command(Domain.PLAYER, Domain.HAND, "PlayerAction",
                f"player: {p.name}\n"
                f"action: CHECK")
            self._event(Domain.HAND, "ActionTaken",
                f"player: {p.name}\n"
                f"action: CHECK\n"
                f"pot: {self.pot}",
                [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

        elif action == "call":
            self._command(Domain.PLAYER, Domain.HAND, "PlayerAction",
                f"player: {p.name}\n"
                f"action: CALL\n"
                f"amount: {result.pot_contribution}")
            self._event(Domain.HAND, "ActionTaken",
                f"player: {p.name}\n"
                f"action: CALL\n"
                f"amount: {result.pot_contribution}\n"
                f"player_stack: {p.stack}\n"
                f"pot: {self.pot}" + (" (ALL-IN)" if p.all_in else ""),
                [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

        elif action == "bet":
            self._command(Domain.PLAYER, Domain.HAND, "PlayerAction",
                f"player: {p.name}\n"
                f"action: BET\n"
                f"amount: {result.pot_contribution}")
            self._event(Domain.HAND, "ActionTaken",
                f"player: {p.name}\n"
                f"action: BET\n"
                f"amount: {result.pot_contribution}\n"
                f"player_stack: {p.stack}\n"
                f"pot: {self.pot}" + (" (ALL-IN)" if p.all_in else ""),
                [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

        elif action == "raise":
            self._command(Domain.PLAYER, Domain.HAND, "PlayerAction",
                f"player: {p.name}\n"
                f"action: RAISE\n"
                f"amount: {result.pot_contribution}")
            self._event(Domain.HAND, "ActionTaken",
                f"player: {p.name}\n"
                f"action: RAISE to {p.bet}\n"
                f"player_stack: {p.stack}\n"
                f"pot: {self.pot}" + (" (ALL-IN)" if p.all_in else ""),
                [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

    # ─────────────────────────────────────────────────────────────
    # COMMUNITY CARDS (Hold'em only)
    # ─────────────────────────────────────────────────────────────

    def deal_community(self, phase: str, count: int):
        """Deal community cards - triggered by ProcessManager."""
        self._pm_reaction("BettingRoundComplete",
            f"transitions to DEALING_COMMUNITY, dispatches DealCommunityCards({count})")

        self._command(Domain.PROCESS_MANAGER, Domain.HAND, "DealCommunityCards",
            f"count: {count}")

        cards = [self.deck.pop() for _ in range(count)]
        self.community.extend(cards)

        self._event(Domain.HAND, "CommunityCardsDealt",
            f"cards: {cards_str(cards)}\n"
            f"phase: {phase}\n"
            f"board: {cards_str(self.community)}",
            [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

        self._pm_reaction("CommunityCardsDealt",
            "transitions to BETTING, dispatches RequestAction")

    # ─────────────────────────────────────────────────────────────
    # DRAW PHASE (uses angzarr DrawRound and FiveCardDrawRules)
    # ─────────────────────────────────────────────────────────────

    def draw_round(self, first_to_act_seat: int):
        """Run draw phase using DrawRound from angzarr."""
        self._pm_reaction("BettingRoundComplete",
            "transitions to DRAW phase, dispatches RequestDraw")

        # Build player states
        player_states = [
            PlayerState(
                seat=p.seat, stack=p.stack, bet=p.bet,
                folded=p.folded, all_in=p.all_in
            )
            for p in self.players.values()
        ]

        draw = DrawRound(player_states, first_to_act_seat)

        while not draw.is_complete():
            seat = draw.get_next_to_draw()
            if seat is None:
                break

            p = self.players[seat]

            # Log request
            self._command(Domain.PROCESS_MANAGER, Domain.PLAYER, "RequestDraw",
                f"player: {p.name}\n"
                f"hole_cards: {cards_str(p.hole_cards)}")

            self._event(Domain.PLAYER, "DrawRequested",
                f"player: {p.name}\n"
                f"player_type: AI\n"
                f"max_discard: 5",
                [Domain.OUTPUT_SAGA])

            self._log(f"")
            self._log(f"   ┌─ AI Draw Decision for {p.name}")
            self._log(f"   │  Current hand: {cards_str(p.hole_cards)}")

            # Get AI decision
            decision = self.ai.decide_draw(p.hole_cards)

            self._log(f"   │  Found: {decision.reasoning}")
            self._log(f"   │  Discard: {len(decision.discard_indices)} card(s)")
            self._log(f"   └─►")

            # Execute draw using FiveCardDrawRules
            discarded = [p.hole_cards[i] for i in decision.discard_indices]

            self._command(Domain.PLAYER, Domain.HAND, "ExecuteDraw",
                f"player: {p.name}\n"
                f"discard_indices: {decision.discard_indices}\n"
                f"discarded: {cards_str(discarded) if discarded else '[]'}")

            # Use rules to execute the draw
            draw_result = self.rules.execute_draw(
                self.deck, p.hole_cards, decision.discard_indices
            )
            p.hole_cards = draw_result.new_hole_cards
            self.deck = draw_result.remaining_deck

            self._event(Domain.HAND, "DrawCompleted",
                f"player: {p.name}\n"
                f"cards_discarded: {len(decision.discard_indices)}\n"
                f"new_cards: {cards_str(draw_result.cards_drawn) if draw_result.cards_drawn else '[]'}\n"
                f"new_hand: {cards_str(p.hole_cards)}",
                [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

        self._event(Domain.HAND, "DrawPhaseComplete",
            f"pot: {self.pot}",
            [Domain.OUTPUT_SAGA, Domain.PROCESS_MANAGER])

        self._pm_reaction("DrawPhaseComplete",
            "transitions to BETTING (post-draw), dispatches RequestAction")

    # ─────────────────────────────────────────────────────────────
    # SHOWDOWN
    # ─────────────────────────────────────────────────────────────

    def showdown(self):
        """Determine winner at showdown."""
        active = [p for p in self.players.values() if not p.folded]

        if len(active) == 1:
            winner = active[0]
            self._log(f"")
            self._log(f"   [All others folded]")
            self._award_pot([winner], [self.pot])
            return

        phase_name = "post-draw" if self.variant == GameVariant.FIVE_CARD_DRAW else "river"
        self._pm_reaction(f"BettingRoundComplete ({phase_name})",
            "transitions to SHOWDOWN, dispatches RevealCards")

        # Reveal cards
        results = []
        for p in active:
            self._command(Domain.PROCESS_MANAGER, Domain.HAND, "RevealCards",
                f"player: {p.name}\n"
                f"muck: false")

            hand_result = self.rules.evaluate_hand(p.hole_cards, self.community)
            hand_name = HAND_NAMES.get(hand_result[0], "Unknown")

            self._event(Domain.HAND, "CardsRevealed",
                f"player: {p.name}\n"
                f"cards: {cards_str(p.hole_cards)}\n"
                f"ranking: {hand_name}\n"
                f"score: {hand_result[1]}",
                [Domain.OUTPUT_SAGA])

            results.append((p, hand_result))

        # Find winner(s)
        results.sort(key=lambda x: x[1][1], reverse=True)
        best_score = results[0][1][1]
        winners = [r[0] for r in results if r[1][1] == best_score]

        if len(winners) == 1:
            self._award_pot(winners, [self.pot])
        else:
            split = self.pot // len(winners)
            self._award_pot(winners, [split] * len(winners))

    def _award_pot(self, winners: list, amounts: list):
        """Award pot to winners."""
        self._pm_reaction("CardsRevealed (all)", "dispatches AwardPot")

        awards_str = "\n".join(f"  {w.name}: {chips(a)}" for w, a in zip(winners, amounts))
        self._command(Domain.PROCESS_MANAGER, Domain.HAND, "AwardPot",
            f"awards:\n{awards_str}")

        winner_info = []
        for w, a in zip(winners, amounts):
            hand_result = self.rules.evaluate_hand(w.hole_cards, self.community)
            hand_name = HAND_NAMES.get(hand_result[0], "Winner")
            winner_info.append(f"  {w.name}: {chips(a)} ({hand_name})")

        self._event(Domain.HAND, "PotAwarded",
            f"winners:\n" + "\n".join(winner_info),
            [Domain.OUTPUT_SAGA, Domain.HAND_RESULTS_SAGA, Domain.PROCESS_MANAGER])

        for w, a in zip(winners, amounts):
            self._saga_reaction(Domain.HAND_RESULTS_SAGA, "PotAwarded",
                f"dispatches DepositFunds({w.name}, {a})")

            self._command(Domain.HAND_RESULTS_SAGA, Domain.PLAYER, "DepositFunds",
                f"player: {w.name}\n"
                f"amount: {a}")

            w.stack += a
            self._event(Domain.PLAYER, "FundsDeposited",
                f"player: {w.name}\n"
                f"amount: {a}\n"
                f"new_balance: {w.stack}",
                [Domain.OUTPUT_SAGA])

        self.pot = 0

        self._event(Domain.HAND, "HandComplete",
            f"hand_number: {self.hand_num}\n"
            f"winners: {[w.name for w in winners]}",
            [Domain.OUTPUT_SAGA, Domain.TABLE_SYNC_SAGA])

        self._saga_reaction(Domain.TABLE_SYNC_SAGA, "HandComplete",
            "dispatches EndHand to Table")

        self._command(Domain.TABLE_SYNC_SAGA, Domain.TABLE, "EndHand",
            f"hand_number: {self.hand_num}")

        stacks_str = "\n".join(
            f"  {p.name}: {chips(p.stack)}"
            for p in sorted(self.players.values(), key=lambda x: x.seat)
        )
        self._event(Domain.TABLE, "HandEnded",
            f"hand_number: {self.hand_num}\n"
            f"final_stacks:\n{stacks_str}",
            [Domain.OUTPUT_SAGA, Domain.HAND_RESULTS_SAGA])

        self._saga_reaction(Domain.HAND_RESULTS_SAGA, "HandEnded",
            "dispatches ReleaseFunds for each player")

        for p in self.players.values():
            self._command(Domain.HAND_RESULTS_SAGA, Domain.PLAYER, "ReleaseFunds",
                f"player: {p.name}\n"
                f"table: main")
            self._event(Domain.PLAYER, "FundsReleased",
                f"player: {p.name}\n"
                f"amount: {p.stack}",
                [Domain.OUTPUT_SAGA])

    # ─────────────────────────────────────────────────────────────
    # HAND FLOW (uses rules.get_next_phase())
    # ─────────────────────────────────────────────────────────────

    def seated_order(self, start_seat=None):
        seats = sorted(self.players.keys())
        if start_seat:
            idx = seats.index(start_seat) if start_seat in seats else 0
            seats = seats[idx:] + seats[:idx]
        return [self.players[s] for s in seats if s in self.players]

    def run_hand(self):
        """Run a complete hand using rules.get_next_phase() for phase transitions."""
        if not self.start_hand():
            return False

        self.deal_cards()
        bb_seat = self.post_blinds()

        seats = sorted(self.players.keys())
        bb_idx = seats.index(bb_seat)
        first_to_act = seats[(bb_idx + 1) % len(seats)]

        # Start with preflop betting
        current_phase = poker_types.PREFLOP
        if not self.betting_round(first_to_act, "PREFLOP"):
            self.showdown()
            return True

        # Get first-to-act after dealer for post-flop phases
        dealer_idx = seats.index(self.dealer_seat)
        first_postflop = self._first_active_after(dealer_idx, seats)

        # Phase loop driven by rules.get_next_phase()
        while True:
            transition = self.rules.get_next_phase(current_phase)
            if transition is None or transition.is_showdown:
                break

            current_phase = transition.next_phase
            phase_name = self._phase_name(current_phase)

            # Handle draw phase specially
            if current_phase == poker_types.DRAW:
                self.draw_round(first_postflop)
                continue

            # Deal community cards if needed
            if transition.community_cards_to_deal > 0:
                self.deal_community(phase_name, transition.community_cards_to_deal)

            # Run betting round
            if not self.betting_round(first_postflop, phase_name):
                self.showdown()
                return True

        self.showdown()
        return True

    def _first_active_after(self, dealer_idx: int, seats: list) -> int:
        """Find first active player after dealer."""
        for i in range(1, len(seats) + 1):
            seat = seats[(dealer_idx + i) % len(seats)]
            if seat in self.players and not self.players[seat].folded:
                return seat
        return seats[0]

    def _phase_name(self, phase: int) -> str:
        """Convert phase enum to display name."""
        names = {
            poker_types.PREFLOP: "PREFLOP",
            poker_types.FLOP: "FLOP",
            poker_types.TURN: "TURN",
            poker_types.RIVER: "RIVER",
            poker_types.DRAW: "DRAW",
            poker_types.SHOWDOWN: "SHOWDOWN",
        }
        return names.get(phase, "UNKNOWN")


def main():
    """Run poker with full event logging."""
    parser = argparse.ArgumentParser(description="Angzarr Poker Engine")
    parser.add_argument('--variant', choices=['holdem', 'draw'], default='holdem',
                        help="Game variant: 'holdem' for Texas Hold'em, 'draw' for Five Card Draw")
    parser.add_argument('--players', type=int, default=6,
                        help="Number of players (2-10)")
    parser.add_argument('--stack', type=int, default=500,
                        help="Starting stack size")
    parser.add_argument('--small-blind', type=int, default=5,
                        help="Small blind amount")
    parser.add_argument('--big-blind', type=int, default=10,
                        help="Big blind amount")
    args = parser.parse_args()

    variant = GameVariant.TEXAS_HOLDEM if args.variant == 'holdem' else GameVariant.FIVE_CARD_DRAW
    variant_name = VARIANT_NAMES[variant]

    log_file = open("hand_log.txt", "w", encoding="utf-8")

    def log(msg=""):
        print(msg)
        log_file.write(msg + "\n")

    log("╔══════════════════════════════════════════════════════════════════════╗")
    log(f"║       ♠ ♥ ♣ ♦  ANGZARR POKER: {variant_name:^20}  ♦ ♣ ♥ ♠       ║")
    log("╚══════════════════════════════════════════════════════════════════════╝")
    log("")
    log("This log shows every COMMAND and EVENT with source/target domains.")
    log("Nothing happens without a triggering event!")
    log("")

    game = EventSourcedPokerGame(
        variant=variant,
        small_blind=args.small_blind,
        big_blind=args.big_blind,
        output=log
    )

    # Setup
    log("=" * 70)
    log("  SETUP PHASE")
    log("=" * 70)

    game.create_table("Main")

    names = ["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry", "Ivy", "Jack"]
    for i in range(min(args.players, 10)):
        game.add_player(names[i], args.stack, seat=i + 1)

    # Run hands until one player remains
    while len([p for p in game.players.values() if p.stack > 0]) > 1:
        if not game.run_hand():
            break

    # Final result
    log("\n" + "=" * 70)
    log("  TOURNAMENT COMPLETE!")
    log("=" * 70)
    for p in game.players.values():
        if p.stack > 0:
            log(f"\n  🏆 WINNER: {p.name} with {chips(p.stack)}")
            break

    log("\n  ♠ ♥ ♣ ♦  GAME OVER  ♦ ♣ ♥ ♠\n")

    log_file.close()
    print(f"\nLog written to: hand_log.txt")


if __name__ == "__main__":
    main()
