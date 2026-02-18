"""Player aggregate - rich domain model."""

from dataclasses import dataclass, field
import logging

from angzarr_client import Aggregate, handles, applies, now
from angzarr_client.compensation import (
    CompensationContext,
    delegate_to_framework,
)
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.angzarr import aggregate_pb2 as aggregate
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player_proto
from angzarr_client.proto.examples import types_pb2 as poker_types

logger = logging.getLogger(__name__)


@dataclass
class _PlayerState:
    """Internal state representation."""

    player_id: str = ""
    display_name: str = ""
    email: str = ""
    player_type: int = 0
    ai_model_id: str = ""
    bankroll: int = 0
    reserved_funds: int = 0
    table_reservations: dict = field(default_factory=dict)
    status: str = ""


class Player(Aggregate[_PlayerState]):
    """Player aggregate with event sourcing."""

    domain = "player"

    def _create_empty_state(self) -> _PlayerState:
        return _PlayerState()

    # --- Event appliers ---

    @applies(player_proto.PlayerRegistered)
    def apply_registered(self, state: _PlayerState, event: player_proto.PlayerRegistered):
        """Apply PlayerRegistered event to state."""
        state.player_id = f"player_{event.email}"
        state.display_name = event.display_name
        state.email = event.email
        state.player_type = event.player_type
        state.ai_model_id = event.ai_model_id
        state.status = "active"
        state.bankroll = 0
        state.reserved_funds = 0

    @applies(player_proto.FundsDeposited)
    def apply_deposited(self, state: _PlayerState, event: player_proto.FundsDeposited):
        """Apply FundsDeposited event to state."""
        if event.new_balance:
            state.bankroll = event.new_balance.amount

    @applies(player_proto.FundsWithdrawn)
    def apply_withdrawn(self, state: _PlayerState, event: player_proto.FundsWithdrawn):
        """Apply FundsWithdrawn event to state."""
        if event.new_balance:
            state.bankroll = event.new_balance.amount

    @applies(player_proto.FundsReserved)
    def apply_reserved(self, state: _PlayerState, event: player_proto.FundsReserved):
        """Apply FundsReserved event to state."""
        if event.new_reserved_balance:
            state.reserved_funds = event.new_reserved_balance.amount
        table_key = event.table_root.hex()
        if event.amount:
            state.table_reservations[table_key] = event.amount.amount

    @applies(player_proto.FundsReleased)
    def apply_released(self, state: _PlayerState, event: player_proto.FundsReleased):
        """Apply FundsReleased event to state."""
        if event.new_reserved_balance:
            state.reserved_funds = event.new_reserved_balance.amount
        table_key = event.table_root.hex()
        state.table_reservations.pop(table_key, None)

    @applies(player_proto.FundsTransferred)
    def apply_transferred(self, state: _PlayerState, event: player_proto.FundsTransferred):
        """Apply FundsTransferred event to state."""
        if event.new_balance:
            state.bankroll = event.new_balance.amount

    # --- State accessors ---

    @property
    def exists(self) -> bool:
        return bool(self._get_state().player_id)

    @property
    def player_id(self) -> str:
        return self._get_state().player_id

    @property
    def display_name(self) -> str:
        return self._get_state().display_name

    @property
    def email(self) -> str:
        return self._get_state().email

    @property
    def player_type(self) -> int:
        return self._get_state().player_type

    @property
    def ai_model_id(self) -> str:
        return self._get_state().ai_model_id

    @property
    def bankroll(self) -> int:
        return self._get_state().bankroll

    @property
    def reserved_funds(self) -> int:
        return self._get_state().reserved_funds

    @property
    def table_reservations(self) -> dict:
        return self._get_state().table_reservations

    @property
    def status(self) -> str:
        return self._get_state().status

    @property
    def available_balance(self) -> int:
        state = self._get_state()
        return state.bankroll - state.reserved_funds

    @property
    def is_ai(self) -> bool:
        return self._get_state().player_type == poker_types.PlayerType.AI

    # --- Command handlers ---

    @handles(player_proto.RegisterPlayer)
    def register(self, cmd: player_proto.RegisterPlayer) -> player_proto.PlayerRegistered:
        """Register a new player."""
        if self.exists:
            raise CommandRejectedError("Player already exists")
        if not cmd.display_name:
            raise CommandRejectedError("display_name is required")
        if not cmd.email:
            raise CommandRejectedError("email is required")

        return player_proto.PlayerRegistered(
            display_name=cmd.display_name,
            email=cmd.email,
            player_type=cmd.player_type,
            ai_model_id=cmd.ai_model_id,
            registered_at=now(),
        )

    @handles(player_proto.DepositFunds)
    def deposit(self, cmd: player_proto.DepositFunds) -> player_proto.FundsDeposited:
        """Deposit funds into player's bankroll."""
        if not self.exists:
            raise CommandRejectedError("Player does not exist")

        amount = cmd.amount.amount if cmd.amount else 0
        if amount <= 0:
            raise CommandRejectedError("amount must be positive")

        new_balance = self.bankroll + amount
        return player_proto.FundsDeposited(
            amount=cmd.amount,
            new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
            deposited_at=now(),
        )

    @handles(player_proto.WithdrawFunds)
    def withdraw(self, cmd: player_proto.WithdrawFunds) -> player_proto.FundsWithdrawn:
        """Withdraw funds from player's bankroll."""
        if not self.exists:
            raise CommandRejectedError("Player does not exist")

        amount = cmd.amount.amount if cmd.amount else 0
        if amount <= 0:
            raise CommandRejectedError("amount must be positive")
        if amount > self.available_balance:
            raise CommandRejectedError("Insufficient funds")

        new_balance = self.bankroll - amount
        return player_proto.FundsWithdrawn(
            amount=cmd.amount,
            new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
            withdrawn_at=now(),
        )

    @handles(player_proto.ReserveFunds)
    def reserve(self, cmd: player_proto.ReserveFunds) -> player_proto.FundsReserved:
        """Reserve funds for a table buy-in."""
        if not self.exists:
            raise CommandRejectedError("Player does not exist")

        amount = cmd.amount.amount if cmd.amount else 0
        if amount <= 0:
            raise CommandRejectedError("amount must be positive")

        table_key = cmd.table_root.hex()
        if table_key in self.table_reservations:
            raise CommandRejectedError("Funds already reserved for this table")

        if amount > self.available_balance:
            raise CommandRejectedError("Insufficient funds")

        new_reserved = self.reserved_funds + amount
        new_available = self.bankroll - new_reserved
        return player_proto.FundsReserved(
            amount=cmd.amount,
            table_root=cmd.table_root,
            new_available_balance=poker_types.Currency(
                amount=new_available, currency_code="CHIPS"
            ),
            new_reserved_balance=poker_types.Currency(
                amount=new_reserved, currency_code="CHIPS"
            ),
            reserved_at=now(),
        )

    @handles(player_proto.ReleaseFunds)
    def release(self, cmd: player_proto.ReleaseFunds) -> player_proto.FundsReleased:
        """Release reserved funds when leaving a table."""
        if not self.exists:
            raise CommandRejectedError("Player does not exist")

        table_key = cmd.table_root.hex()
        reserved_for_table = self.table_reservations.get(table_key, 0)
        if reserved_for_table == 0:
            raise CommandRejectedError("No funds reserved for this table")

        new_reserved = self.reserved_funds - reserved_for_table
        new_available = self.bankroll - new_reserved
        return player_proto.FundsReleased(
            amount=poker_types.Currency(amount=reserved_for_table, currency_code="CHIPS"),
            table_root=cmd.table_root,
            new_available_balance=poker_types.Currency(
                amount=new_available, currency_code="CHIPS"
            ),
            new_reserved_balance=poker_types.Currency(
                amount=new_reserved, currency_code="CHIPS"
            ),
            released_at=now(),
        )

    # --- Saga/PM Compensation ---

    def handle_revocation(self, notification: types.Notification) -> aggregate.BusinessResponse:
        """Handle rejection for player-related saga/PM failures.

        Called when a saga/PM command targeting another aggregate is rejected.
        For example, if ReserveFunds succeeded but the subsequent table join failed,
        this method handles compensation.

        Uses CompensationContext helpers for cleaner code.
        """
        ctx = CompensationContext.from_notification(notification)

        logger.warning(
            "Player compensation: issuer=%s reason=%s seq=%d",
            ctx.issuer_name,
            ctx.rejection_reason,
            ctx.source_event_sequence,
        )

        # Example: Auto-release funds if a table join saga failed
        # if "table" in ctx.issuer_name.lower():
        #     # Extract table_root from rejected command
        #     table_root = ...
        #     event = player_proto.FundsReleased(
        #         table_root=table_root,
        #         amount=self.table_reservations.get(table_root.hex(), 0),
        #         released_at=now(),
        #     )
        #     self._apply_and_record(event)
        #     return emit_compensation_events(self.event_book())

        # Default: delegate to framework
        return delegate_to_framework(
            reason=f"Player aggregate: no custom compensation for {ctx.issuer_name}"
        )
