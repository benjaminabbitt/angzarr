"""Output projector gRPC service.

Subscribes to events from player, table, and hand domains and writes
formatted game logs to hand_log.txt.
"""

import os
import sys
from pathlib import Path

import structlog

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.projector_handler import ProjectorHandler, run_projector_server

from projector import OutputProjector

structlog.configure(
    processors=[
        structlog.stdlib.add_log_level,
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.JSONRenderer(),
    ],
    wrapper_class=structlog.make_filtering_bound_logger(0),
    context_class=dict,
    logger_factory=structlog.PrintLoggerFactory(),
)

logger = structlog.get_logger()

# Output file path
LOG_FILE = os.environ.get("HAND_LOG_FILE", "hand_log.txt")


class FileLogProjector:
    """Projector that writes events to a log file."""

    def __init__(self, log_path: str):
        self.log_path = log_path
        self.log_file = None
        self.projector = OutputProjector(
            output_fn=self._write_line,
            show_timestamps=True,
        )

    def _write_line(self, text: str) -> None:
        """Write a line to the log file."""
        if self.log_file is None:
            self.log_file = open(self.log_path, "a", encoding="utf-8")
        self.log_file.write(text + "\n")
        self.log_file.flush()

    def handle(self, event_book: types.EventBook) -> types.Projection:
        """Handle an event book and return a projection."""
        self.projector.handle_event_book(event_book)

        # Return a projection with the sequence number
        seq = 0
        if event_book.pages:
            page = event_book.pages[-1]
            if page.WhichOneof("sequence") == "num":
                seq = page.num

        return types.Projection(
            cover=event_book.cover,
            projector="output",
            sequence=seq,
        )

    def close(self):
        """Close the log file."""
        if self.log_file is not None:
            self.log_file.close()
            self.log_file = None


def main():
    """Run the output projector gRPC service."""
    # Clear the log file at startup
    log_path = Path(LOG_FILE)
    if log_path.exists():
        log_path.unlink()

    file_projector = FileLogProjector(LOG_FILE)

    # Create handler that subscribes to all poker domains
    handler = ProjectorHandler(
        "output",
        "player",
        "table",
        "hand",
    ).with_handle(file_projector.handle)

    logger.info(
        "output_projector_starting",
        log_file=LOG_FILE,
        domains=["player", "table", "hand"],
    )

    try:
        run_projector_server(
            name="output",
            default_port="50490",
            handler=handler,
            logger=logger,
        )
    finally:
        file_projector.close()


if __name__ == "__main__":
    main()
