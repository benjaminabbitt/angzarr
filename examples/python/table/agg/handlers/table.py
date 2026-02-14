"""Table aggregate - rich domain model."""

import uuid
from dataclasses import dataclass, field
from typing import Optional

from angzarr_client import Aggregate, handles, now
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import table_pb2 as table_proto


@dataclass
class _SeatState:
    """State of a single seat at the table."""
    position: int
    player_root: bytes
    stack: int
    is_active: bool = True
    is_sitting_out: bool = False


@dataclass
class _TableState:
    """Internal state representation."""
    table_id: str = ""
    table_name: str = ""
    game_variant: int = 0
    small_blind: int = 0
    big_blind: int = 0
    min_buy_in: int = 0
    max_buy_in: int = 0
    max_players: int = 9
    action_timeout_seconds: int = 30
    seats: dict = field(default_factory=dict)  # position -> _SeatState
    dealer_position: int = 0
    hand_count: int = 0
    current_hand_root: bytes = b""
    status: str = ""


class Table(Aggregate[_TableState]):
    """Table aggregate with event sourcing."""

    domain = "table"

    def _create_empty_state(self) -> _TableState:
        return _TableState()

    def _apply_event(self, state: _TableState, event_any) -> None:
        """Apply a single event to state."""
        type_url = event_any.type_url

        if type_url.endswith("TableCreated"):
            event = table_proto.TableCreated()
            event_any.Unpack(event)
            state.table_id = f"table_{event.table_name}"
            state.table_name = event.table_name
            state.game_variant = event.game_variant
            state.small_blind = event.small_blind
            state.big_blind = event.big_blind
            state.min_buy_in = event.min_buy_in
            state.max_buy_in = event.max_buy_in
            state.max_players = event.max_players
            state.action_timeout_seconds = event.action_timeout_seconds
            state.status = "waiting"

        elif type_url.endswith("PlayerJoined"):
            event = table_proto.PlayerJoined()
            event_any.Unpack(event)
            state.seats[event.seat_position] = _SeatState(
                position=event.seat_position,
                player_root=event.player_root,
                stack=event.stack,
            )

        elif type_url.endswith("PlayerLeft"):
            event = table_proto.PlayerLeft()
            event_any.Unpack(event)
            state.seats.pop(event.seat_position, None)

        elif type_url.endswith("PlayerSatOut"):
            event = table_proto.PlayerSatOut()
            event_any.Unpack(event)
            for seat in state.seats.values():
                if seat.player_root == event.player_root:
                    seat.is_sitting_out = True
                    break

        elif type_url.endswith("PlayerSatIn"):
            event = table_proto.PlayerSatIn()
            event_any.Unpack(event)
            for seat in state.seats.values():
                if seat.player_root == event.player_root:
                    seat.is_sitting_out = False
                    break

        elif type_url.endswith("HandStarted"):
            event = table_proto.HandStarted()
            event_any.Unpack(event)
            state.hand_count = event.hand_number
            state.current_hand_root = event.hand_root
            state.dealer_position = event.dealer_position
            state.status = "in_hand"

        elif type_url.endswith("HandEnded"):
            event = table_proto.HandEnded()
            event_any.Unpack(event)
            state.current_hand_root = b""
            state.status = "waiting"
            # Apply stack changes
            for player_hex, delta in event.stack_changes.items():
                player_root = bytes.fromhex(player_hex)
                for seat in state.seats.values():
                    if seat.player_root == player_root:
                        seat.stack += delta
                        break

        elif type_url.endswith("ChipsAdded"):
            event = table_proto.ChipsAdded()
            event_any.Unpack(event)
            for seat in state.seats.values():
                if seat.player_root == event.player_root:
                    seat.stack = event.new_stack
                    break

    # --- State accessors ---

    @property
    def exists(self) -> bool:
        return bool(self._get_state().table_id)

    @property
    def table_id(self) -> str:
        return self._get_state().table_id

    @property
    def table_name(self) -> str:
        return self._get_state().table_name

    @property
    def game_variant(self) -> int:
        return self._get_state().game_variant

    @property
    def small_blind(self) -> int:
        return self._get_state().small_blind

    @property
    def big_blind(self) -> int:
        return self._get_state().big_blind

    @property
    def min_buy_in(self) -> int:
        return self._get_state().min_buy_in

    @property
    def max_buy_in(self) -> int:
        return self._get_state().max_buy_in

    @property
    def max_players(self) -> int:
        return self._get_state().max_players

    @property
    def seats(self) -> dict:
        return self._get_state().seats

    @property
    def dealer_position(self) -> int:
        return self._get_state().dealer_position

    @property
    def hand_count(self) -> int:
        return self._get_state().hand_count

    @property
    def current_hand_root(self) -> bytes:
        return self._get_state().current_hand_root

    @property
    def status(self) -> str:
        return self._get_state().status

    @property
    def player_count(self) -> int:
        return len(self._get_state().seats)

    @property
    def active_player_count(self) -> int:
        return sum(1 for s in self._get_state().seats.values() if not s.is_sitting_out)

    @property
    def is_full(self) -> bool:
        state = self._get_state()
        return len(state.seats) >= state.max_players

    def get_seat(self, position: int) -> Optional[_SeatState]:
        return self._get_state().seats.get(position)

    def find_player_seat(self, player_root: bytes) -> Optional[_SeatState]:
        for seat in self._get_state().seats.values():
            if seat.player_root == player_root:
                return seat
        return None

    def _find_available_seat(self, preferred: int = -1) -> Optional[int]:
        state = self._get_state()
        # preferred_seat > 0 means explicit seat preference (proto3 defaults to 0)
        if preferred > 0 and preferred < state.max_players:
            if preferred not in state.seats:
                return preferred
        for pos in range(state.max_players):
            if pos not in state.seats:
                return pos
        return None

    def _next_dealer_position(self) -> int:
        state = self._get_state()
        if not state.seats:
            return 0
        positions = sorted(state.seats.keys())
        current_idx = 0
        for i, pos in enumerate(positions):
            if pos == state.dealer_position:
                current_idx = i
                break
        next_idx = (current_idx + 1) % len(positions)
        return positions[next_idx]

    # --- Command handlers ---

    @handles(table_proto.CreateTable)
    def create(self, cmd: table_proto.CreateTable) -> table_proto.TableCreated:
        """Create a new table."""
        if self.exists:
            raise CommandRejectedError("Table already exists")
        if not cmd.table_name:
            raise CommandRejectedError("table_name is required")
        if cmd.small_blind <= 0:
            raise CommandRejectedError("small_blind must be positive")
        if cmd.big_blind <= 0:
            raise CommandRejectedError("big_blind must be positive")
        if cmd.big_blind < cmd.small_blind:
            raise CommandRejectedError("big_blind must be >= small_blind")
        if cmd.max_players < 2 or cmd.max_players > 10:
            raise CommandRejectedError("max_players must be between 2 and 10")

        return table_proto.TableCreated(
            table_name=cmd.table_name,
            game_variant=cmd.game_variant,
            small_blind=cmd.small_blind,
            big_blind=cmd.big_blind,
            min_buy_in=cmd.min_buy_in or cmd.big_blind * 20,
            max_buy_in=cmd.max_buy_in or cmd.big_blind * 100,
            max_players=cmd.max_players or 9,
            action_timeout_seconds=cmd.action_timeout_seconds or 30,
            created_at=now(),
        )

    @handles(table_proto.JoinTable)
    def join(self, cmd: table_proto.JoinTable) -> table_proto.PlayerJoined:
        """Add a player to the table."""
        if not self.exists:
            raise CommandRejectedError("Table does not exist")
        if not cmd.player_root:
            raise CommandRejectedError("player_root is required")
        if self.find_player_seat(cmd.player_root):
            raise CommandRejectedError("Player already seated at table")
        if self.is_full:
            raise CommandRejectedError("Table is full")
        if cmd.buy_in_amount < self.min_buy_in:
            raise CommandRejectedError(f"Buy-in must be at least {self.min_buy_in}")
        if cmd.buy_in_amount > self.max_buy_in:
            raise CommandRejectedError(f"Buy-in cannot exceed {self.max_buy_in}")
        # preferred_seat -1 means no preference; 0+ means specific seat
        if cmd.preferred_seat > 0 and self.get_seat(cmd.preferred_seat) is not None:
            raise CommandRejectedError("Seat is occupied")

        seat_position = self._find_available_seat(cmd.preferred_seat)

        return table_proto.PlayerJoined(
            player_root=cmd.player_root,
            seat_position=seat_position,
            buy_in_amount=cmd.buy_in_amount,
            stack=cmd.buy_in_amount,
            joined_at=now(),
        )

    @handles(table_proto.LeaveTable)
    def leave(self, cmd: table_proto.LeaveTable) -> table_proto.PlayerLeft:
        """Remove a player from the table."""
        if not self.exists:
            raise CommandRejectedError("Table does not exist")
        if not cmd.player_root:
            raise CommandRejectedError("player_root is required")

        seat = self.find_player_seat(cmd.player_root)
        if not seat:
            raise CommandRejectedError("Player is not seated at table")
        if self.status == "in_hand":
            raise CommandRejectedError("Cannot leave table during a hand")

        return table_proto.PlayerLeft(
            player_root=cmd.player_root,
            seat_position=seat.position,
            chips_cashed_out=seat.stack,
            left_at=now(),
        )

    @handles(table_proto.StartHand)
    def start_hand(self, cmd: table_proto.StartHand) -> table_proto.HandStarted:
        """Start a new hand."""
        if not self.exists:
            raise CommandRejectedError("Table does not exist")
        if self.status == "in_hand":
            raise CommandRejectedError("Hand already in progress")
        if self.active_player_count < 2:
            raise CommandRejectedError("Not enough players to start hand")

        state = self._get_state()

        # Generate hand root
        hand_number = state.hand_count + 1
        hand_uuid = uuid.uuid5(
            uuid.NAMESPACE_DNS,
            f"angzarr.poker.hand.{state.table_id}.{hand_number}",
        )
        hand_root = hand_uuid.bytes

        # Advance dealer button
        dealer_position = self._next_dealer_position()

        # Get active player positions
        active_positions = sorted(
            pos for pos, seat in state.seats.items() if not seat.is_sitting_out
        )

        # Find blind positions
        dealer_idx = 0
        for i, pos in enumerate(active_positions):
            if pos == dealer_position:
                dealer_idx = i
                break

        if len(active_positions) == 2:
            sb_position = active_positions[dealer_idx]
            bb_position = active_positions[(dealer_idx + 1) % 2]
        else:
            sb_position = active_positions[(dealer_idx + 1) % len(active_positions)]
            bb_position = active_positions[(dealer_idx + 2) % len(active_positions)]

        # Build active players list
        active_players = []
        for pos in active_positions:
            seat = state.seats[pos]
            active_players.append(
                table_proto.SeatSnapshot(
                    position=pos,
                    player_root=seat.player_root,
                    stack=seat.stack,
                )
            )

        event = table_proto.HandStarted(
            hand_root=hand_root,
            hand_number=hand_number,
            dealer_position=dealer_position,
            small_blind_position=sb_position,
            big_blind_position=bb_position,
            game_variant=state.game_variant,
            small_blind=state.small_blind,
            big_blind=state.big_blind,
            started_at=now(),
        )
        event.active_players.extend(active_players)

        return event

    @handles(table_proto.EndHand)
    def end_hand(self, cmd: table_proto.EndHand) -> table_proto.HandEnded:
        """End the current hand."""
        if not self.exists:
            raise CommandRejectedError("Table does not exist")
        if self.status != "in_hand":
            raise CommandRejectedError("No hand in progress")
        if cmd.hand_root != self.current_hand_root:
            raise CommandRejectedError("Hand root mismatch")

        # Calculate stack changes from results
        stack_changes = {}
        for result in cmd.results:
            player_hex = result.winner_root.hex()
            if player_hex not in stack_changes:
                stack_changes[player_hex] = 0
            stack_changes[player_hex] += result.amount

        event = table_proto.HandEnded(
            hand_root=cmd.hand_root,
            stack_changes=stack_changes,
            ended_at=now(),
        )
        event.results.extend(cmd.results)

        return event
