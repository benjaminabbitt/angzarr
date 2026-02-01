"""Tests for customer client logic."""

from google.protobuf.any_pb2 import Any as AnyProto
from proto import domains_pb2 as domains
from angzarr import angzarr_pb2 as angzarr
from customer_logic import CustomerLogic


def _create_command_book(command, domain: str = "customer") -> angzarr.CommandBook:
    """Helper to wrap a command in a CommandBook."""
    command_any = AnyProto()
    command_any.Pack(command, type_url_prefix="type.examples/")

    return angzarr.CommandBook(
        cover=angzarr.Cover(domain=domain),
        pages=[angzarr.CommandPage(sequence=0, command=command_any)],
    )


def _create_contextual_command(
    command_book: angzarr.CommandBook,
    prior_events: angzarr.EventBook | None = None,
) -> angzarr.ContextualCommand:
    """Helper to create a ContextualCommand."""
    return angzarr.ContextualCommand(
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
