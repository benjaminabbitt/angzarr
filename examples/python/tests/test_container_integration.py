"""Container integration tests for angzarr system.

These tests verify the deployed angzarr system works correctly by connecting
to the gateway and testing command execution and event queries.
"""

import os
import uuid

import grpc
import pytest
from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import angzarr_pb2 as angzarr
from angzarr import angzarr_pb2_grpc as angzarr_grpc
from examples import domains_pb2 as domains


def uuid_to_proto(u: uuid.UUID) -> angzarr.UUID:
    """Convert Python UUID to proto UUID."""
    return angzarr.UUID(value=u.bytes)


def proto_to_uuid(p: angzarr.UUID) -> uuid.UUID:
    """Convert proto UUID to Python UUID."""
    return uuid.UUID(bytes=p.value)


class AngzarrClient:
    """Client for interacting with angzarr gateway and services."""

    def __init__(self, gateway_address: str):
        self.gateway_address = gateway_address
        self.channel = grpc.insecure_channel(gateway_address)
        self.command_gateway = angzarr_grpc.CommandGatewayStub(self.channel)
        # EventQuery is on the entity sidecar, use NodePort mapped to hostPort 50052
        entity_address = gateway_address.replace("50051", "50052")
        self.entity_channel = grpc.insecure_channel(entity_address)
        self.event_query = angzarr_grpc.EventQueryStub(self.entity_channel)

    def close(self):
        self.channel.close()
        self.entity_channel.close()

    def execute_command(
        self, domain: str, root: uuid.UUID, command
    ) -> angzarr.CommandResponse:
        """Execute a command and return the response."""
        command_any = AnyProto()
        command_any.Pack(command, type_url_prefix="type.examples/")

        command_book = angzarr.CommandBook(
            cover=angzarr.Cover(domain=domain, root=uuid_to_proto(root)),
            pages=[angzarr.CommandPage(sequence=0, command=command_any)],
        )

        return self.command_gateway.Execute(command_book)

    def query_events(self, domain: str, root: uuid.UUID) -> angzarr.EventBook:
        """Query events for an aggregate."""
        query = angzarr.Query(
            domain=domain,
            root=uuid_to_proto(root),
            lower_bound=0,
        )
        return self.event_query.GetEventBook(query)


@pytest.fixture
def client(gateway_address: str):
    """Create angzarr client for tests."""
    client = AngzarrClient(gateway_address)
    yield client
    client.close()


@pytest.fixture
def customer_id() -> uuid.UUID:
    """Generate a new customer ID for each test."""
    return uuid.uuid4()


@pytest.mark.skipif(
    os.environ.get("ANGZARR_TEST_MODE") != "container",
    reason="Container tests require ANGZARR_TEST_MODE=container",
)
class TestContainerIntegration:
    """Integration tests that run against a deployed angzarr cluster."""

    def test_create_customer_via_gateway(self, client: AngzarrClient, customer_id: uuid.UUID):
        """Test creating a customer produces CustomerCreated event."""
        cmd = domains.CreateCustomer(name="Container Test", email="container@test.com")

        response = client.execute_command("customer", customer_id, cmd)

        assert response is not None, "Expected a response"
        assert response.events is not None, "Expected events in response"
        assert len(response.events.pages) > 0, "Expected at least one event page"

        last_event = response.events.pages[-1]
        assert "CustomerCreated" in last_event.event.type_url, (
            f"Expected CustomerCreated event, got {last_event.event.type_url}"
        )

    def test_query_customer_events(self, client: AngzarrClient, customer_id: uuid.UUID):
        """Test querying events after creating a customer."""
        # Create customer first
        cmd = domains.CreateCustomer(name="Query Test", email="query@test.com")
        response = client.execute_command("customer", customer_id, cmd)
        assert response is not None, "Failed to create customer"
        assert len(response.events.pages) > 0, "Expected events from command"

        # Query events
        event_book = client.query_events("customer", customer_id)

        assert event_book is not None, "Expected event book from query"
        assert len(event_book.pages) == 1, f"Expected 1 event, got {len(event_book.pages)}"

        # Verify event type
        assert "CustomerCreated" in event_book.pages[0].event.type_url


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
