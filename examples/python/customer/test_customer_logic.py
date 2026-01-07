"""Tests for customer business logic."""

from google.protobuf.any_pb2 import Any as AnyProto
from proto import domains_pb2 as domains
from evented.proto import evented_pb2 as evented
from customer_logic import CustomerLogic


def _create_command_book(command, domain: str = "customer") -> evented.CommandBook:
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


class TestCustomerLogic:
    """Tests for CustomerLogic."""

    def test_create_customer_success(self):
        """Creating a customer produces CustomerCreated event."""
        logic = CustomerLogic()

        cmd = domains.CreateCustomer(name="Alice", email="alice@example.com")
        command_book = _create_command_book(cmd)
        ctx_cmd = _create_contextual_command(command_book)

        result = logic.handle(ctx_cmd)

        assert len(result.pages) == 1
        assert result.pages[0].event.type_url.endswith("CustomerCreated")

        event = domains.CustomerCreated()
        result.pages[0].event.Unpack(event)
        assert event.name == "Alice"
        assert event.email == "alice@example.com"

    def test_add_loyalty_points_requires_existing_customer(self):
        """Adding points to non-existent customer fails."""
        logic = CustomerLogic()

        cmd = domains.AddLoyaltyPoints(points=100, reason="signup bonus")
        command_book = _create_command_book(cmd)
        ctx_cmd = _create_contextual_command(command_book)

        try:
            logic.handle(ctx_cmd)
            assert False, "Expected ValueError"
        except ValueError as e:
            assert "does not exist" in str(e)
