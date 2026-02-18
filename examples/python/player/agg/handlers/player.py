"""Player aggregate - rich domain model.

DOC: This file is referenced in docs/docs/examples/aggregates.mdx
     Update documentation when making changes to handler patterns.
"""

from dataclasses import dataclass, field
import logging

from angzarr_client import Aggregate, handles, applies, now, rejected
from angzarr_client.compensation import (
    CompensationContext,
    delegate_to_framework,
    emit_compensation_events,
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

    # docs:start:state_appliers_oo
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
    # docs:end:state_appliers_oo

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

    # docs:start:annotation_handlers
    # --- Deposit: guard/validate/compute ---

    # docs:start:deposit_guard
    @staticmethod
    def _guard_deposit(state: _PlayerState) -> None:
        if not state.player_id:
            raise CommandRejectedError("Player does not exist")
    # docs:end:deposit_guard

    # docs:start:deposit_validate
    @staticmethod
    def _validate_deposit(cmd: player_proto.DepositFunds) -> int:
        amount = cmd.amount.amount if cmd.amount else 0
        if amount <= 0:
            raise CommandRejectedError("amount must be positive")
        return amount
    # docs:end:deposit_validate

    # docs:start:deposit_compute
    @staticmethod
    def _compute_deposit(
        cmd: player_proto.DepositFunds, state: _PlayerState, amount: int
    ) -> player_proto.FundsDeposited:
        new_balance = state.bankroll + amount
        return player_proto.FundsDeposited(
            amount=cmd.amount,
            new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
            deposited_at=now(),
        )
    # docs:end:deposit_compute

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
        state = self._get_state()
        self._guard_deposit(state)
        amount = self._validate_deposit(cmd)
        return self._compute_deposit(cmd, state, amount)

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

    # docs:start:reserve_funds_oo
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
    # docs:end:reserve_funds_oo

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
    # docs:end:annotation_handlers

    # --- Saga/PM Compensation ---
    # docs:start:rejected_handler

    @rejected(domain="table", command="JoinTable")
    def handle_join_rejected(
        self, notification: types.Notification
    ) -> player_proto.FundsReleased:
        """Release reserved funds when table join fails.

        Called when the JoinTable command (issued by saga-player-table after
        FundsReserved) is rejected by the Table aggregate.
        """
        ctx = CompensationContext.from_notification(notification)

        logger.warning(
            "Player compensation for JoinTable rejection: reason=%s",
            ctx.rejection_reason,
        )

        # Extract table_root from the rejected command
        table_root = b""
        if ctx.rejected_command and ctx.rejected_command.cover:
            table_root = ctx.rejected_command.cover.root.value

        # Release the funds that were reserved for this table
        reserved_amount = self.table_reservations.get(table_root.hex(), 0)
        new_reserved = self.reserved_funds - reserved_amount
        new_available = self.bankroll - new_reserved

        return player_proto.FundsReleased(
            amount=poker_types.Currency(amount=reserved_amount, currency_code="CHIPS"),
            table_root=table_root,
            reason=f"Join failed: {ctx.rejection_reason}",
            new_available_balance=poker_types.Currency(
                amount=new_available, currency_code="CHIPS"
            ),
            new_reserved_balance=poker_types.Currency(
                amount=new_reserved, currency_code="CHIPS"
            ),
            released_at=now(),
        )

    # docs:end:rejected_handler
