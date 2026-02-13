"""Betting round management for poker."""

from dataclasses import dataclass, field
from typing import Optional, List, Callable


@dataclass
class PlayerState:
    """State of a player in a betting round."""

    seat: int
    stack: int
    bet: int = 0
    folded: bool = False
    all_in: bool = False
    acted: bool = False


@dataclass
class ActionResult:
    """Result of processing a player action."""

    raised: bool  # True if action was a raise/bet
    hand_continues: bool  # True if more than one player remains
    pot_contribution: int  # Amount added to pot


class BettingRound:
    """
    Manages a single betting round.

    Tracks which players need to act, processes actions,
    and determines when the round is complete.
    """

    def __init__(
        self,
        players: List[PlayerState],
        first_to_act_seat: int,
        current_bet: int = 0,
        pot: int = 0,
    ):
        """
        Initialize betting round.

        Args:
            players: List of PlayerState objects
            first_to_act_seat: Seat number of first player to act
            current_bet: Current bet amount (e.g., big blind)
            pot: Current pot size
        """
        self.players = {p.seat: p for p in players}
        self.current_bet = current_bet
        self.pot = pot
        self.last_raiser: Optional[int] = None

        # Build action order starting from first_to_act
        seats = sorted(self.players.keys())
        if first_to_act_seat not in seats:
            first_to_act_seat = seats[0]
        start_idx = seats.index(first_to_act_seat)
        self._action_order = seats[start_idx:] + seats[:start_idx]

        # Track who still needs to act
        self._needs_to_act = set(
            s for s in self._action_order
            if not self.players[s].folded and not self.players[s].all_in
        )

        self._position = 0
        self._max_iterations = len(seats) * 10

    def get_next_to_act(self) -> Optional[int]:
        """
        Get seat of next player who needs to act.

        Returns:
            Seat number or None if round is complete
        """
        if not self._needs_to_act:
            return None

        iterations = 0
        while iterations < self._max_iterations:
            iterations += 1
            seat = self._action_order[self._position % len(self._action_order)]
            self._position += 1

            if seat in self._needs_to_act:
                p = self.players[seat]
                if not p.folded and not p.all_in:
                    return seat
                else:
                    self._needs_to_act.discard(seat)

        return None

    def get_to_call(self, seat: int) -> int:
        """Get amount player needs to call."""
        return self.current_bet - self.players[seat].bet

    def process_action(self, seat: int, action: str, amount: int = 0) -> ActionResult:
        """
        Process a player's action.

        Args:
            seat: Player's seat number
            action: One of "fold", "check", "call", "bet", "raise"
            amount: Amount for bet/raise/call

        Returns:
            ActionResult with outcome information
        """
        p = self.players[seat]
        raised = False
        pot_contribution = 0

        if action == "fold":
            p.folded = True

        elif action == "check":
            pass  # No change

        elif action == "call":
            call_amt = min(self.current_bet - p.bet, p.stack)
            p.stack -= call_amt
            p.bet += call_amt
            self.pot += call_amt
            pot_contribution = call_amt
            if p.stack == 0:
                p.all_in = True

        elif action == "bet":
            actual = min(amount, p.stack)
            p.stack -= actual
            p.bet += actual
            self.pot += actual
            pot_contribution = actual
            self.current_bet = p.bet
            if p.stack == 0:
                p.all_in = True
            raised = True
            self.last_raiser = seat

        elif action == "raise":
            actual = min(amount, p.stack)
            p.stack -= actual
            p.bet += actual
            self.pot += actual
            pot_contribution = actual
            self.current_bet = p.bet
            if p.stack == 0:
                p.all_in = True
            raised = True
            self.last_raiser = seat

        # Mark player as acted
        p.acted = True
        self._needs_to_act.discard(seat)

        # If raise, reopen action for others
        if raised:
            for s in self._action_order:
                if s != seat and not self.players[s].folded and not self.players[s].all_in:
                    self._needs_to_act.add(s)

        # Check if hand continues
        active_count = len([x for x in self.players.values() if not x.folded])

        return ActionResult(
            raised=raised,
            hand_continues=(active_count > 1),
            pot_contribution=pot_contribution,
        )

    def is_complete(self) -> bool:
        """Check if betting round is complete."""
        return len(self._needs_to_act) == 0

    def active_players(self) -> List[PlayerState]:
        """Get list of players who haven't folded."""
        return [p for p in self.players.values() if not p.folded]

    def reset_for_new_round(self):
        """Reset player bets for a new betting round."""
        for p in self.players.values():
            p.bet = 0
            p.acted = False

        self.current_bet = 0
        self._needs_to_act = set(
            s for s in self._action_order
            if not self.players[s].folded and not self.players[s].all_in
        )
        self._position = 0


class DrawRound:
    """
    Manages the draw phase in Five Card Draw.

    Coordinates draw requests and executions for each player.
    """

    def __init__(self, players: List[PlayerState], first_to_act_seat: int):
        """
        Initialize draw round.

        Args:
            players: List of PlayerState objects
            first_to_act_seat: Seat number of first player to draw
        """
        self.players = {p.seat: p for p in players}

        # Build action order starting from first_to_act
        seats = sorted(self.players.keys())
        if first_to_act_seat not in seats:
            first_to_act_seat = seats[0]
        start_idx = seats.index(first_to_act_seat)
        self._action_order = seats[start_idx:] + seats[:start_idx]

        # Players who need to draw
        self._pending = [
            s for s in self._action_order
            if not self.players[s].folded and not self.players[s].all_in
        ]
        self._index = 0

    def get_next_to_draw(self) -> Optional[int]:
        """
        Get seat of next player who needs to draw.

        Returns:
            Seat number or None if draw phase is complete
        """
        if self._index >= len(self._pending):
            return None
        seat = self._pending[self._index]
        self._index += 1
        return seat

    def is_complete(self) -> bool:
        """Check if draw phase is complete."""
        return self._index >= len(self._pending)
