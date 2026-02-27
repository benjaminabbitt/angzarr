"""Player-to-Table saga: propagates player intent as table facts.

Flow:
- Player receives SitOut command → emits PlayerSittingOut event
- This saga receives PlayerSittingOut → emits PlayerSatOut fact to table
- Table aggregate accepts the fact (no validation)

Same pattern for SitIn/PlayerReturningToPlay → PlayerSatIn.
"""

from google.protobuf.any_pb2 import Any as ProtoAny

from angzarr_client import now
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.saga import Saga, domain, handles, output_domain


@domain("player")
@output_domain("table")
class PlayerTableSaga(Saga):
    """Saga that propagates player sit-out intent to table as facts.

    Player owns the intent to sit out/in. The table aggregate must accept
    these as facts (no validation) because player has authority over their
    own participation state.
    """

    name = "saga-player-table"

    def __init__(self) -> None:
        super().__init__()
        self._current_root: bytes = b""

    def dispatch(
        self,
        event_any,
        root: bytes = None,
        correlation_id: str = "",
        destinations: list[types.EventBook] = None,
    ) -> list[types.CommandBook]:
        """Override to store source root for handler access."""
        self._current_root = root or b""
        return super().dispatch(event_any, root, correlation_id, destinations)

    @handles(player.PlayerSittingOut)
    def handle_sitting_out(
        self,
        event: player.PlayerSittingOut,
        destinations: list[types.EventBook] = None,
    ) -> None:
        """Propagate PlayerSittingOut as PlayerSatOut fact to table."""
        fact = table.PlayerSatOut(
            player_root=self._current_root,
            sat_out_at=event.sat_out_at or now(),
        )
        self._emit_table_fact(
            fact, event.table_root, f"sitout-{self._current_root.hex()}"
        )
        return None

    @handles(player.PlayerReturningToPlay)
    def handle_returning_to_play(
        self,
        event: player.PlayerReturningToPlay,
        destinations: list[types.EventBook] = None,
    ) -> None:
        """Propagate PlayerReturningToPlay as PlayerSatIn fact to table."""
        fact = table.PlayerSatIn(
            player_root=self._current_root,
            sat_in_at=event.sat_in_at or now(),
        )
        self._emit_table_fact(
            fact, event.table_root, f"sitin-{self._current_root.hex()}"
        )
        return None

    def _emit_table_fact(self, fact, table_root: bytes, external_id: str) -> None:
        """Emit a fact to the table aggregate."""
        cover = types.Cover(
            domain="table",
            root=types.UUID(value=table_root),
            external_id=external_id,
        )

        fact_any = ProtoAny()
        fact_any.Pack(fact, type_url_prefix="type.googleapis.com/")

        fact_book = types.EventBook(
            cover=cover,
            pages=[types.EventPage(event=fact_any)],
        )

        self.emit_event(fact_book)
