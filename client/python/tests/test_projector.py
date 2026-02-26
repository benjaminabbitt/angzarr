"""Tests for Projector ABC and @handles decorator.

Tests both OO (class-based) and protocol-based (router) patterns.
Uses consistent domains: order, inventory, fulfillment, player.
Includes SQLite-based projectors using SQLAlchemy Core 2.0.
"""

import pytest
from google.protobuf import any_pb2
from sqlalchemy import (
    Column,
    DateTime,
    Engine,
    Integer,
    MetaData,
    String,
    Table,
    create_engine,
    insert,
    select,
    text,
)

from angzarr_client.handler_protocols import ProjectorDomainHandler
from angzarr_client.projector import Projector, domain, handles
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.router import ProjectorRouter

from .fixtures import (
    PlayerRegistered,
    ScoreUpdated,
    ShipmentCreated,
    StockReserved,
    StockUpdated,
)

# =============================================================================
# SQLAlchemy Core schema definitions
# =============================================================================

metadata = MetaData()

audit_trail = Table(
    "audit_trail",
    metadata,
    Column("id", Integer, primary_key=True, autoincrement=True),
    Column("event_type", String(100), nullable=False),
    Column("aggregate_id", String(100), nullable=False),
    Column("payload", String(1000)),
    Column("occurred_at", String(50)),
)

player_stats = Table(
    "player_stats",
    metadata,
    Column("player_id", String(100), primary_key=True),
    Column("display_name", String(100)),
    Column("total_score", Integer, default=0),
    Column("games_played", Integer, default=0),
)


def create_test_db() -> Engine:
    """Create an in-memory SQLite database with schema."""
    engine = create_engine("sqlite:///:memory:", echo=False)
    metadata.create_all(engine)
    return engine


# =============================================================================
# OO Pattern: SQLite-based Projectors
# =============================================================================


@domain("inventory")
class AuditTrailProjector(Projector):
    """Projects events to audit trail table using SQLAlchemy Core.

    Demonstrates SQLite projector with type-safe queries.
    """

    name = "projector-audit-trail"

    def __init__(self, engine: Engine):
        self._engine = engine

    @handles(StockUpdated)
    def project_stock_updated(self, event: StockUpdated) -> types.Projection:
        with self._engine.connect() as conn:
            conn.execute(
                insert(audit_trail).values(
                    event_type="StockUpdated",
                    aggregate_id=event.sku,
                    payload=f"quantity={event.quantity}",
                    occurred_at="now",
                )
            )
            conn.commit()
        return types.Projection(projector=self.name)

    @handles(StockReserved)
    def project_stock_reserved(self, event: StockReserved) -> types.Projection:
        with self._engine.connect() as conn:
            conn.execute(
                insert(audit_trail).values(
                    event_type="StockReserved",
                    aggregate_id=event.sku,
                    payload=f"order={event.order_id},qty={event.quantity}",
                    occurred_at="now",
                )
            )
            conn.commit()
        return types.Projection(projector=self.name)


@domain("player")
class PlayerStatsProjector(Projector):
    """Projects player events to stats table using SQLAlchemy Core.

    Demonstrates upsert pattern with SQLite.
    """

    name = "projector-player-stats"

    def __init__(self, engine: Engine):
        self._engine = engine

    @handles(PlayerRegistered)
    def project_registered(self, event: PlayerRegistered) -> types.Projection:
        with self._engine.connect() as conn:
            conn.execute(
                insert(player_stats).values(
                    player_id=event.player_id,
                    display_name=event.display_name,
                    total_score=0,
                    games_played=0,
                )
            )
            conn.commit()
        return types.Projection(projector=self.name)

    @handles(ScoreUpdated)
    def project_score(self, event: ScoreUpdated) -> types.Projection:
        with self._engine.connect() as conn:
            # Update existing player stats
            conn.execute(
                text("""
                    UPDATE player_stats
                    SET total_score = :new_total, games_played = games_played + 1
                    WHERE player_id = :player_id
                    """),
                {"new_total": event.new_total, "player_id": event.player_id},
            )
            conn.commit()
        return types.Projection(projector=self.name)


@domain("inventory")
class NoopProjector(Projector):
    """Projector that returns empty projection."""

    name = "projector-noop"

    @handles(StockUpdated)
    def project_stock(self, event: StockUpdated) -> None:
        return None


# =============================================================================
# Protocol-based Pattern: ProjectorRouter with ProjectorDomainHandler
# =============================================================================


class AuditTrailProjectorHandler(ProjectorDomainHandler):
    """Protocol-based audit trail projector handler."""

    def __init__(self, engine: Engine):
        self._engine = engine

    def event_types(self) -> list[str]:
        return ["StockUpdated"]

    def project(self, events: types.EventBook) -> types.Projection:
        # Process each event in the EventBook
        for page in events.pages:
            if page.HasField("event") and page.event.type_url.endswith("StockUpdated"):
                stock_updated = StockUpdated()
                stock_updated.ParseFromString(page.event.value)
                with self._engine.connect() as conn:
                    conn.execute(
                        insert(audit_trail).values(
                            event_type="StockUpdated",
                            aggregate_id=stock_updated.sku,
                            payload=f"quantity={stock_updated.quantity}",
                            occurred_at="now",
                        )
                    )
                    conn.commit()
        return types.Projection(projector="projector-audit-trail-fn")


class FulfillmentProjectorHandler(ProjectorDomainHandler):
    """Protocol-based fulfillment projector handler."""

    def event_types(self) -> list[str]:
        return ["ShipmentCreated"]

    def project(self, events: types.EventBook) -> types.Projection:
        return types.Projection(projector="projector-fulfillment-tracking")


def build_audit_trail_router(engine: Engine) -> ProjectorRouter:
    """Build protocol-based audit trail projector."""
    handler = AuditTrailProjectorHandler(engine)
    return ProjectorRouter("projector-audit-trail-fn", "inventory", handler)


def build_fulfillment_projector_router() -> ProjectorRouter:
    """Projects fulfillment events (no DB, for comparison)."""
    handler = FulfillmentProjectorHandler()
    return ProjectorRouter("projector-fulfillment-tracking", "fulfillment", handler)


# =============================================================================
# Tests for @handles decorator
# =============================================================================


class TestProjectsDecorator:
    def test_decorator_marks_handler(self):
        # Use NoopProjector for basic decorator tests
        method = NoopProjector.project_stock
        assert hasattr(method, "_is_handler")
        assert method._is_handler is True
        assert method._event_type == StockUpdated

    def test_decorator_validates_missing_param(self):
        with pytest.raises(TypeError, match="must have cmd parameter"):

            @handles(StockUpdated)
            def bad_method(self):
                pass

    def test_decorator_validates_missing_type_hint(self):
        with pytest.raises(TypeError, match="missing type hint"):

            @handles(StockUpdated)
            def bad_method(self, event):
                pass

    def test_decorator_validates_type_hint_mismatch(self):
        with pytest.raises(TypeError, match="doesn't match type hint"):

            @handles(StockUpdated)
            def bad_method(self, event: StockReserved):
                pass

    def test_decorator_preserves_function_name(self):
        method = NoopProjector.project_stock
        assert method.__name__ == "project_stock"


# =============================================================================
# Tests for Projector subclass validation
# =============================================================================


class TestProjectorValidation:
    def test_missing_name_raises(self):
        with pytest.raises(TypeError, match="must define 'name'"):

            @domain("inventory")
            class BadProjector(Projector):
                @handles(StockUpdated)
                def handle(self, event: StockUpdated):
                    pass

    def test_missing_input_domain_raises(self):
        """Lazy validation: error raised at first use, not definition."""

        class BadProjector(Projector):
            name = "bad-projector"

            @handles(StockUpdated)
            def project_stock(self, event: StockUpdated):
                pass

        # Error raised at first use (handle classmethod)
        with pytest.raises(TypeError, match="must use @domain decorator"):
            BadProjector.handle(types.EventBook())

    def test_duplicate_handler_raises(self):
        with pytest.raises(TypeError, match="duplicate handler"):

            @domain("inventory")
            class BadProjector(Projector):
                name = "bad-projector"

                @handles(StockUpdated)
                def handle_one(self, event: StockUpdated):
                    pass

                @handles(StockUpdated)
                def handle_two(self, event: StockUpdated):
                    pass


# =============================================================================
# Tests for SQLite Audit Trail Projector
# =============================================================================


class TestAuditTrailProjector:
    def test_projects_stock_updated_to_audit_trail(self):
        engine = create_test_db()
        projector = AuditTrailProjector(engine)

        event = StockUpdated(sku="SKU-123", quantity=100)
        event_any = any_pb2.Any()
        event_any.Pack(event)

        projection = projector.dispatch(event_any)

        assert projection.projector == "projector-audit-trail"

        # Verify data in SQLite
        with engine.connect() as conn:
            result = conn.execute(select(audit_trail)).fetchall()
            assert len(result) == 1
            assert result[0].event_type == "StockUpdated"
            assert result[0].aggregate_id == "SKU-123"

    def test_projects_stock_reserved_to_audit_trail(self):
        engine = create_test_db()
        projector = AuditTrailProjector(engine)

        event = StockReserved(order_id="order-1", sku="SKU-A", quantity=5)
        event_any = any_pb2.Any()
        event_any.Pack(event)

        projector.dispatch(event_any)

        with engine.connect() as conn:
            result = conn.execute(select(audit_trail)).fetchall()
            assert len(result) == 1
            assert result[0].event_type == "StockReserved"
            assert "order=order-1" in result[0].payload

    def test_multiple_events_append_to_audit_trail(self):
        engine = create_test_db()
        projector = AuditTrailProjector(engine)

        for i in range(3):
            event = StockUpdated(sku=f"SKU-{i}", quantity=i * 10)
            event_any = any_pb2.Any()
            event_any.Pack(event)
            projector.dispatch(event_any)

        with engine.connect() as conn:
            result = conn.execute(select(audit_trail)).fetchall()
            assert len(result) == 3


# =============================================================================
# Tests for SQLite Player Stats Projector
# =============================================================================


class TestPlayerStatsProjector:
    def test_projects_player_registered(self):
        engine = create_test_db()
        projector = PlayerStatsProjector(engine)

        event = PlayerRegistered(
            player_id="player-1",
            display_name="Alice",
            registered_at="2024-01-15",
        )
        event_any = any_pb2.Any()
        event_any.Pack(event)

        projection = projector.dispatch(event_any)

        assert projection.projector == "projector-player-stats"

        with engine.connect() as conn:
            result = conn.execute(
                select(player_stats).where(player_stats.c.player_id == "player-1")
            ).fetchone()
            assert result.display_name == "Alice"
            assert result.total_score == 0
            assert result.games_played == 0

    def test_projects_score_updated(self):
        engine = create_test_db()
        projector = PlayerStatsProjector(engine)

        # First register player
        reg_event = PlayerRegistered(
            player_id="player-1", display_name="Bob", registered_at="2024-01-15"
        )
        reg_any = any_pb2.Any()
        reg_any.Pack(reg_event)
        projector.dispatch(reg_any)

        # Then update score
        score_event = ScoreUpdated(
            player_id="player-1", game_id="game-1", score_delta=100, new_total=100
        )
        score_any = any_pb2.Any()
        score_any.Pack(score_event)
        projector.dispatch(score_any)

        with engine.connect() as conn:
            result = conn.execute(
                select(player_stats).where(player_stats.c.player_id == "player-1")
            ).fetchone()
            assert result.total_score == 100
            assert result.games_played == 1

    def test_multiple_score_updates(self):
        engine = create_test_db()
        projector = PlayerStatsProjector(engine)

        # Register
        reg_event = PlayerRegistered(
            player_id="player-1", display_name="Charlie", registered_at="2024-01-15"
        )
        reg_any = any_pb2.Any()
        reg_any.Pack(reg_event)
        projector.dispatch(reg_any)

        # Play 3 games
        for i in range(3):
            score_event = ScoreUpdated(
                player_id="player-1",
                game_id=f"game-{i}",
                score_delta=50,
                new_total=(i + 1) * 50,
            )
            score_any = any_pb2.Any()
            score_any.Pack(score_event)
            projector.dispatch(score_any)

        with engine.connect() as conn:
            result = conn.execute(
                select(player_stats).where(player_stats.c.player_id == "player-1")
            ).fetchone()
            assert result.total_score == 150
            assert result.games_played == 3


# =============================================================================
# Tests for protocol-based pattern (router)
# =============================================================================


class TestProtocolBasedProjectorRouter:
    def test_router_writes_to_sqlite(self):
        engine = create_test_db()
        router = build_audit_trail_router(engine)

        event = StockUpdated(sku="SKU-fn-123", quantity=42)
        event_any = any_pb2.Any()
        event_any.Pack(event)

        source = types.EventBook(
            cover=types.Cover(domain="inventory"),
            pages=[types.EventPage(event=event_any)],
        )
        projection = router.dispatch(source)

        assert projection.projector == "projector-audit-trail-fn"

        # Verify data in SQLite
        with engine.connect() as conn:
            result = conn.execute(select(audit_trail)).fetchall()
            assert len(result) == 1
            assert result[0].aggregate_id == "SKU-fn-123"


# =============================================================================
# Tests comparing both patterns produce equivalent output
# =============================================================================


class TestPatternEquivalence:
    """Verify OO and protocol-based patterns both write to SQLite correctly."""

    def test_same_db_state_for_stock_updated(self):
        # OO pattern
        oo_engine = create_test_db()
        oo_projector = AuditTrailProjector(oo_engine)

        event = StockUpdated(sku="SKU-eq", quantity=99)
        event_any = any_pb2.Any()
        event_any.Pack(event)
        oo_projector.dispatch(event_any)

        # Protocol pattern
        router_engine = create_test_db()
        router = build_audit_trail_router(router_engine)
        source = types.EventBook(
            cover=types.Cover(domain="inventory"),
            pages=[types.EventPage(event=event_any)],
        )
        router.dispatch(source)

        # Both produce same DB state
        with oo_engine.connect() as conn:
            oo_rows = conn.execute(select(audit_trail)).fetchall()

        with router_engine.connect() as conn:
            router_rows = conn.execute(select(audit_trail)).fetchall()

        assert len(oo_rows) == len(router_rows) == 1
        assert oo_rows[0].event_type == router_rows[0].event_type == "StockUpdated"
        assert oo_rows[0].aggregate_id == router_rows[0].aggregate_id == "SKU-eq"
