"""Transaction bounded context business logic.

Handles purchases, discounts, and transaction lifecycle.
"""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains


class TransactionLogic:
    """Business logic for Transaction aggregate."""

    DOMAIN = "transaction"

    def handle(self, contextual_command: angzarr.ContextualCommand) -> angzarr.EventBook:
        """Process a command and return resulting events."""
        command_book = contextual_command.command
        prior_events = contextual_command.events

        # Rebuild current state from events
        state = self._rebuild_state(prior_events)

        # Get the command from the first page
        if not command_book.pages:
            raise ValueError("CommandBook has no pages")

        command_page = command_book.pages[0]
        command_any = command_page.command

        # Route to appropriate handler based on command type
        if command_any.type_url.endswith("CreateTransaction"):
            return self._handle_create_transaction(command_book, command_any, state)
        elif command_any.type_url.endswith("ApplyDiscount"):
            return self._handle_apply_discount(command_book, command_any, state)
        elif command_any.type_url.endswith("CompleteTransaction"):
            return self._handle_complete_transaction(command_book, command_any, state)
        elif command_any.type_url.endswith("CancelTransaction"):
            return self._handle_cancel_transaction(command_book, command_any, state)
        else:
            raise ValueError(f"Unknown command type: {command_any.type_url}")

    def _rebuild_state(self, event_book: angzarr.EventBook | None) -> domains.TransactionState:
        """Rebuild transaction state from events."""
        state = domains.TransactionState(status="new")

        if event_book is None or not event_book.pages:
            return state

        for page in event_book.pages:
            if not page.event:
                continue

            if page.event.type_url.endswith("TransactionCreated"):
                event = domains.TransactionCreated()
                page.event.Unpack(event)
                state.customer_id = event.customer_id
                state.items.extend(event.items)
                state.subtotal_cents = event.subtotal_cents
                state.status = "pending"

            elif page.event.type_url.endswith("DiscountApplied"):
                event = domains.DiscountApplied()
                page.event.Unpack(event)
                state.discount_cents = event.discount_cents
                state.discount_type = event.discount_type

            elif page.event.type_url.endswith("TransactionCompleted"):
                state.status = "completed"

            elif page.event.type_url.endswith("TransactionCancelled"):
                state.status = "cancelled"

        return state

    def _handle_create_transaction(
        self,
        command_book: angzarr.CommandBook,
        command_any: Any,
        state: domains.TransactionState,
    ) -> angzarr.EventBook:
        """Handle CreateTransaction command."""
        if state.status != "new":
            raise ValueError("Transaction already exists")

        cmd = domains.CreateTransaction()
        command_any.Unpack(cmd)

        if not cmd.customer_id:
            raise ValueError("customer_id is required")
        if not cmd.items:
            raise ValueError("at least one item is required")

        subtotal = sum(item.quantity * item.unit_price_cents for item in cmd.items)

        event = domains.TransactionCreated(
            customer_id=cmd.customer_id,
            items=cmd.items,
            subtotal_cents=subtotal,
            created_at=self._now(),
        )

        event_any = Any()
        event_any.Pack(event, type_url_prefix="type.examples/")

        return angzarr.EventBook(
            cover=command_book.cover,
            pages=[
                angzarr.EventPage(
                    num=0,
                    event=event_any,
                    created_at=self._now(),
                )
            ],
        )

    def _handle_apply_discount(
        self,
        command_book: angzarr.CommandBook,
        command_any: Any,
        state: domains.TransactionState,
    ) -> angzarr.EventBook:
        """Handle ApplyDiscount command."""
        if state.status != "pending":
            raise ValueError("Can only apply discount to pending transaction")

        cmd = domains.ApplyDiscount()
        command_any.Unpack(cmd)

        # Calculate discount
        if cmd.discount_type == "percentage":
            if cmd.value < 0 or cmd.value > 100:
                raise ValueError("Percentage must be 0-100")
            discount_cents = (state.subtotal_cents * cmd.value) // 100
        elif cmd.discount_type == "fixed":
            discount_cents = min(cmd.value, state.subtotal_cents)
        elif cmd.discount_type == "coupon":
            discount_cents = 500  # $5 off
        else:
            raise ValueError(f"Unknown discount type: {cmd.discount_type}")

        event = domains.DiscountApplied(
            discount_type=cmd.discount_type,
            value=cmd.value,
            discount_cents=discount_cents,
            coupon_code=cmd.coupon_code,
        )

        event_any = Any()
        event_any.Pack(event, type_url_prefix="type.examples/")

        return angzarr.EventBook(
            cover=command_book.cover,
            pages=[
                angzarr.EventPage(
                    num=0,
                    event=event_any,
                    created_at=self._now(),
                )
            ],
        )

    def _handle_complete_transaction(
        self,
        command_book: angzarr.CommandBook,
        command_any: Any,
        state: domains.TransactionState,
    ) -> angzarr.EventBook:
        """Handle CompleteTransaction command."""
        if state.status != "pending":
            raise ValueError("Can only complete pending transaction")

        cmd = domains.CompleteTransaction()
        command_any.Unpack(cmd)

        final_total = max(0, state.subtotal_cents - state.discount_cents)
        loyalty_points = final_total // 100  # 1 point per dollar

        event = domains.TransactionCompleted(
            final_total_cents=final_total,
            payment_method=cmd.payment_method,
            loyalty_points_earned=loyalty_points,
            completed_at=self._now(),
        )

        event_any = Any()
        event_any.Pack(event, type_url_prefix="type.examples/")

        return angzarr.EventBook(
            cover=command_book.cover,
            pages=[
                angzarr.EventPage(
                    num=0,
                    event=event_any,
                    created_at=self._now(),
                )
            ],
        )

    def _handle_cancel_transaction(
        self,
        command_book: angzarr.CommandBook,
        command_any: Any,
        state: domains.TransactionState,
    ) -> angzarr.EventBook:
        """Handle CancelTransaction command."""
        if state.status != "pending":
            raise ValueError("Can only cancel pending transaction")

        cmd = domains.CancelTransaction()
        command_any.Unpack(cmd)

        event = domains.TransactionCancelled(
            reason=cmd.reason,
            cancelled_at=self._now(),
        )

        event_any = Any()
        event_any.Pack(event, type_url_prefix="type.examples/")

        return angzarr.EventBook(
            cover=command_book.cover,
            pages=[
                angzarr.EventPage(
                    num=0,
                    event=event_any,
                    created_at=self._now(),
                )
            ],
        )

    def _now(self) -> Timestamp:
        """Get current timestamp."""
        ts = Timestamp()
        ts.FromDatetime(datetime.now(timezone.utc))
        return ts


def get_domains() -> list[str]:
    """Return list of domains this logic handles."""
    return [TransactionLogic.DOMAIN]


if __name__ == "__main__":
    logic = TransactionLogic()

    # Test CreateTransaction
    create_cmd = domains.CreateTransaction(
        customer_id="cust123",
        items=[
            domains.LineItem(
                product_id="prod1",
                name="Widget",
                quantity=2,
                unit_price_cents=999,
            ),
        ],
    )

    cmd_any = Any()
    cmd_any.Pack(create_cmd, type_url_prefix="type.examples/")

    command_book = angzarr.CommandBook(
        cover=angzarr.Cover(domain="transaction"),
        pages=[angzarr.CommandPage(sequence=0, command=cmd_any)],
    )

    ctx_cmd = angzarr.ContextualCommand(command=command_book)
    result = logic.handle(ctx_cmd)

    print(f"Created transaction with {len(result.pages)} event(s)")
    event = domains.TransactionCreated()
    result.pages[0].event.Unpack(event)
    print(f"  customer_id: {event.customer_id}")
    print(f"  subtotal: ${event.subtotal_cents / 100:.2f}")
