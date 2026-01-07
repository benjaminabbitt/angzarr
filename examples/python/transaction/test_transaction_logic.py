"""Tests for transaction business logic."""

from google.protobuf.any_pb2 import Any as AnyProto
from proto import domains_pb2 as domains
from evented.proto import evented_pb2 as evented
from transaction_logic import TransactionLogic


def _create_command_book(command, domain: str = "transaction") -> evented.CommandBook:
    """Helper to wrap a command in a CommandBook."""
    command_any = AnyProto()
    command_any.Pack(command, type_url_prefix="type.examples/")

    return evented.CommandBook(
        cover=evented.Cover(domain=domain),
        pages=[evented.CommandPage(sequence=0, command=command_any)],
    )


def _create_contextual_command(
    command_book: evented.CommandBook,
    prior_events: evented.EventBook | None = None,
) -> evented.ContextualCommand:
    """Helper to create a ContextualCommand."""
    return evented.ContextualCommand(
        command=command_book,
        events=prior_events,
    )


class TestTransactionLogic:
    """Tests for TransactionLogic."""

    def test_create_transaction_success(self):
        """Creating a transaction produces TransactionCreated event."""
        logic = TransactionLogic()

        cmd = domains.CreateTransaction(
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
        command_book = _create_command_book(cmd)
        ctx_cmd = _create_contextual_command(command_book)

        result = logic.handle(ctx_cmd)

        assert len(result.pages) == 1
        assert result.pages[0].event.type_url.endswith("TransactionCreated")

        event = domains.TransactionCreated()
        result.pages[0].event.Unpack(event)
        assert event.customer_id == "cust123"
        assert event.subtotal_cents == 1998  # 2 * 999

    def test_create_transaction_requires_customer_id(self):
        """Creating transaction without customer_id fails."""
        logic = TransactionLogic()

        cmd = domains.CreateTransaction(
            items=[
                domains.LineItem(name="Widget", quantity=1, unit_price_cents=999),
            ],
        )
        command_book = _create_command_book(cmd)
        ctx_cmd = _create_contextual_command(command_book)

        try:
            logic.handle(ctx_cmd)
            assert False, "Expected ValueError"
        except ValueError as e:
            assert "customer_id" in str(e)

    def test_complete_transaction_calculates_loyalty_points(self):
        """Completing transaction calculates loyalty points."""
        logic = TransactionLogic()

        # First create the transaction
        create_cmd = domains.CreateTransaction(
            customer_id="cust123",
            items=[
                domains.LineItem(
                    product_id="prod1",
                    name="Widget",
                    quantity=10,
                    unit_price_cents=1000,  # $10 each
                ),
            ],
        )
        create_book = _create_command_book(create_cmd)
        create_result = logic.handle(_create_contextual_command(create_book))

        # Now complete it
        complete_cmd = domains.CompleteTransaction(payment_method="card")
        complete_book = _create_command_book(complete_cmd)
        ctx_cmd = _create_contextual_command(complete_book, create_result)

        result = logic.handle(ctx_cmd)

        event = domains.TransactionCompleted()
        result.pages[0].event.Unpack(event)
        assert event.final_total_cents == 10000  # $100
        assert event.loyalty_points_earned == 100  # 1 point per dollar
        assert event.payment_method == "card"
