"""Tests for customer log projector."""

from proto import domains_pb2 as domains
from log_projector import CustomerLogProjector, projector_name, projector_domains


class TestCustomerLogProjector:
    """Tests for CustomerLogProjector."""

    def test_projector_name(self):
        """Projector has correct name."""
        assert projector_name() == "log-customer"

    def test_projector_domains(self):
        """Projector listens to customer domain only."""
        domains_list = projector_domains()
        assert domains_list == ["customer"]

    def test_project_customer_created(self, capsys):
        """Project handles CustomerCreated event."""
        projector = CustomerLogProjector()

        customer_created = domains.CustomerCreated(
            name="Test User",
            email="test@example.com",
        )

        event_book = {
            "cover": {
                "domain": "customer",
                "root": {"value": b"\x01\x02\x03\x04\x05\x06\x07\x08"},
            },
            "pages": [
                {
                    "sequence": {"num": 0},
                    "event": {
                        "type_url": "type.examples/examples.CustomerCreated",
                        "value": customer_created.SerializeToString(),
                    },
                }
            ],
        }

        projector.project(event_book)

        captured = capsys.readouterr()
        assert "CUSTOMER" in captured.out
        assert "CustomerCreated" in captured.out
        assert "Test User" in captured.out
