"""Tests for transaction log projector."""

from proto import domains_pb2 as domains
from log_projector import TransactionLogProjector, projector_name, projector_domains


class TestTransactionLogProjector:
    """Tests for TransactionLogProjector."""

    def test_projector_name(self):
        """Projector has correct name."""
        assert projector_name() == "log-transaction"

    def test_projector_domains(self):
        """Projector listens to transaction domain only."""
        domains_list = projector_domains()
        assert domains_list == ["transaction"]

    def test_project_transaction_completed(self, capsys):
        """Project handles TransactionCompleted event."""
        projector = TransactionLogProjector()

        tx_completed = domains.TransactionCompleted(
            final_total_cents=1999,
            payment_method="card",
            loyalty_points_earned=19,
        )

        event_book = {
            "cover": {
                "domain": "transaction",
                "root": {"value": b"\x01\x02\x03\x04\x05\x06\x07\x08"},
            },
            "pages": [
                {
                    "sequence": {"num": 0},
                    "event": {
                        "type_url": "type.examples/examples.TransactionCompleted",
                        "value": tx_completed.SerializeToString(),
                    },
                }
            ],
        }

        projector.project(event_book)

        captured = capsys.readouterr()
        assert "TRANSACTION" in captured.out
        assert "TransactionCompleted" in captured.out
        assert "19.99" in captured.out
