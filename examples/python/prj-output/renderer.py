"""Text renderer for poker events."""

from angzarr_client.helpers import bytes_to_uuid_text
from angzarr_client.proto.examples import types_pb2 as poker_types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import hand_pb2 as hand

SUIT_SYMBOLS = {
    poker_types.CLUBS: "c",
    poker_types.DIAMONDS: "d",
    poker_types.HEARTS: "h",
    poker_types.SPADES: "s",
}

RANK_SYMBOLS = {
    2: "2",
    3: "3",
    4: "4",
    5: "5",
    6: "6",
    7: "7",
    8: "8",
    9: "9",
    10: "T",
    11: "J",
    12: "Q",
    13: "K",
    14: "A",
}

ACTION_SYMBOLS = {
    poker_types.FOLD: "folds",
    poker_types.CHECK: "checks",
    poker_types.CALL: "calls",
    poker_types.BET: "bets",
    poker_types.RAISE: "raises to",
    poker_types.ALL_IN: "all-in",
}

HAND_RANK_NAMES = {
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

PHASE_NAMES = {
    poker_types.PREFLOP: "Preflop",
    poker_types.FLOP: "Flop",
    poker_types.TURN: "Turn",
    poker_types.RIVER: "River",
    poker_types.DRAW: "Draw",
    poker_types.SHOWDOWN: "Showdown",
}


def format_card(card) -> str:
    """Format a card as a string like 'As'."""
    rank = RANK_SYMBOLS.get(card.rank, "?")
    suit = SUIT_SYMBOLS.get(card.suit, "?")
    return f"{rank}{suit}"


def format_cards(cards) -> str:
    """Format a list of cards as a string like '[As Kh]'."""
    if not cards:
        return "[]"
    card_strs = [format_card(c) for c in cards]
    return f"[{' '.join(card_strs)}]"


def format_chips(amount: int) -> str:
    """Format chip amount with commas."""
    return f"${amount:,}"


def format_player_id(player_root: bytes) -> str:
    """Format player ID as standard UUID text format."""
    return bytes_to_uuid_text(player_root)


class TextRenderer:
    """Renders poker events as text output."""

    def __init__(self):
        self.player_names = {}  # player_root -> name
        self.output_lines = []

    def set_player_name(self, player_root: bytes, name: str):
        """Set display name for a player."""
        self.player_names[player_root] = name

    def get_player_name(self, player_root: bytes) -> str:
        """Get display name for a player."""
        if player_root in self.player_names:
            return self.player_names[player_root]
        return f"Player_{format_player_id(player_root)}"

    def render(self, event_type: str, event) -> str:
        """Render an event to text. Returns the rendered string."""
        method = getattr(self, f"_render_{event_type}", None)
        if method:
            return method(event)
        return f"[Unknown event: {event_type}]"

    # Player events

    def _render_PlayerRegistered(self, event: player.PlayerRegistered) -> str:
        name = event.display_name or format_player_id(b"")
        self.player_names[b""] = name  # Will be overwritten with actual root
        return f"* {name} registered"

    def _render_FundsDeposited(self, event: player.FundsDeposited) -> str:
        return f"  Deposited {format_chips(event.amount.amount)} (balance: {format_chips(event.new_balance.amount)})"

    def _render_FundsWithdrawn(self, event: player.FundsWithdrawn) -> str:
        return f"  Withdrew {format_chips(event.amount.amount)} (balance: {format_chips(event.new_balance.amount)})"

    def _render_FundsReserved(self, event: player.FundsReserved) -> str:
        return f"  Reserved {format_chips(event.amount.amount)} for table"

    def _render_FundsReleased(self, event: player.FundsReleased) -> str:
        return f"  Released {format_chips(event.amount.amount)}"

    # Table events

    def _render_TableCreated(self, event: table.TableCreated) -> str:
        variant_name = poker_types.GameVariant.Name(event.game_variant)
        return (
            f"Table '{event.table_name}' created\n"
            f"   Game: {variant_name}\n"
            f"   Blinds: {format_chips(event.small_blind)}/{format_chips(event.big_blind)}\n"
            f"   Buy-in: {format_chips(event.min_buy_in)} - {format_chips(event.max_buy_in)}"
        )

    def _render_PlayerJoined(self, event: table.PlayerJoined) -> str:
        name = self.get_player_name(event.player_root)
        return f"  {name} joined at seat {event.seat_position} with {format_chips(event.buy_in_amount)}"

    def _render_PlayerLeft(self, event: table.PlayerLeft) -> str:
        name = self.get_player_name(event.player_root)
        return f"  {name} left table with {format_chips(event.chips_cashed_out)}"

    def _render_HandStarted(self, event: table.HandStarted) -> str:
        lines = [
            f"\n{'='*60}",
            f"HAND #{event.hand_number}",
            f"{'='*60}",
            f"Dealer: Seat {event.dealer_position}",
            f"Blinds: {format_chips(event.small_blind)}/{format_chips(event.big_blind)}",
            "",
            "Players:",
        ]
        for p in event.active_players:
            name = self.get_player_name(p.player_root)
            position_marker = ""
            if p.position == event.dealer_position:
                position_marker = " (D)"
            elif p.position == event.small_blind_position:
                position_marker = " (SB)"
            elif p.position == event.big_blind_position:
                position_marker = " (BB)"
            lines.append(
                f"  Seat {p.position}: {name} - {format_chips(p.stack)}{position_marker}"
            )
        return "\n".join(lines)

    def _render_HandEnded(self, event: table.HandEnded) -> str:
        lines = ["", "Results:"]
        for result in event.results:
            name = self.get_player_name(result.winner_root)
            lines.append(f"  {name} wins {format_chips(result.amount)}")
        lines.append(f"{'='*60}\n")
        return "\n".join(lines)

    # Hand events

    def _render_CardsDealt(self, event: hand.CardsDealt) -> str:
        lines = ["", "Cards dealt:"]
        for pc in event.player_cards:
            name = self.get_player_name(pc.player_root)
            cards = format_cards(pc.cards)
            lines.append(f"  {name}: {cards}")
        return "\n".join(lines)

    def _render_BlindPosted(self, event: hand.BlindPosted) -> str:
        name = self.get_player_name(event.player_root)
        blind_name = event.blind_type.upper()
        return f"  {name} posts {blind_name} {format_chips(event.amount)}"

    def _render_ActionTaken(self, event: hand.ActionTaken) -> str:
        name = self.get_player_name(event.player_root)
        action_str = ACTION_SYMBOLS.get(event.action, "acts")
        if event.action in (
            poker_types.BET,
            poker_types.RAISE,
            poker_types.CALL,
            poker_types.ALL_IN,
        ):
            return f"  {name} {action_str} {format_chips(event.amount)} (pot: {format_chips(event.pot_total)})"
        return f"  {name} {action_str}"

    def _render_BettingRoundComplete(self, event: hand.BettingRoundComplete) -> str:
        phase = PHASE_NAMES.get(event.completed_phase, "unknown")
        return f"\n--- {phase} complete (pot: {format_chips(event.pot_total)}) ---"

    def _render_CommunityCardsDealt(self, event: hand.CommunityCardsDealt) -> str:
        phase = PHASE_NAMES.get(event.phase, "")
        new_cards = format_cards(event.cards)
        all_cards = format_cards(event.all_community_cards)
        return f"\n  {phase}: {new_cards}\n   Board: {all_cards}"

    def _render_DrawCompleted(self, event: hand.DrawCompleted) -> str:
        name = self.get_player_name(event.player_root)
        return f"  {name} draws {event.cards_drawn} cards"

    def _render_ShowdownStarted(self, event: hand.ShowdownStarted) -> str:
        return "\nSHOWDOWN"

    def _render_CardsRevealed(self, event: hand.CardsRevealed) -> str:
        name = self.get_player_name(event.player_root)
        cards = format_cards(event.cards)
        rank = HAND_RANK_NAMES.get(event.ranking.rank_type, "unknown")
        return f"  {name} shows {cards} - {rank}"

    def _render_CardsMucked(self, event: hand.CardsMucked) -> str:
        name = self.get_player_name(event.player_root)
        return f"  {name} mucks"

    def _render_PotAwarded(self, event: hand.PotAwarded) -> str:
        lines = []
        for winner in event.winners:
            name = self.get_player_name(winner.player_root)
            lines.append(f"  {name} wins {format_chips(winner.amount)}")
        return "\n".join(lines)

    def _render_HandComplete(self, event: hand.HandComplete) -> str:
        lines = ["", "Final stacks:"]
        for stack in event.final_stacks:
            name = self.get_player_name(stack.player_root)
            status = ""
            if stack.has_folded:
                status = " (folded)"
            lines.append(f"  {name}: {format_chips(stack.stack)}{status}")
        return "\n".join(lines)

    def _render_PlayerTimedOut(self, event: hand.PlayerTimedOut) -> str:
        name = self.get_player_name(event.player_root)
        action = ACTION_SYMBOLS.get(event.default_action, "times out")
        return f"  {name} timed out - auto {action}"
