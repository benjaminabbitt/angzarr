"""Tests for Projector ABC and @projects decorator.

Tests both OO (class-based) and function-based (router) patterns.
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

from angzarr_client.projector import Projector, projects
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.router import EventRouter, event_handler

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


class AuditTrailProjector(Projector):
    """Projects events to audit trail table using SQLAlchemy Core.

    Demonstrates SQLite projector with type-safe queries.
    """

    name = "projector-audit-trail"
    input_domain = "inventory"

    def __init__(self, engine: Engine):
        self._engine = engine

    @projects(StockUpdated)
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

    @projects(StockReserved)
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


class PlayerStatsProjector(Projector):
    """Projects player events to stats table using SQLAlchemy Core.

    Demonstrates upsert pattern with SQLite.
    """

    name = "projector-player-stats"
    input_domain = "player"

    def __init__(self, engine: Engine):
        self._engine = engine

    @projects(PlayerRegistered)
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

    @projects(ScoreUpdated)
    def project_score(self, event: ScoreUpdated) -> types.Projection:
        with self._engine.connect() as conn:
            # Update existing player stats
            conn.execute(
                text(
                    """
                    UPDATE player_stats
                    SET total_score = :new_total, games_played = games_played + 1
                    WHERE player_id = :player_id
                    """
                ),
                {"new_total": event.new_total, "player_id": event.player_id},
            )
            conn.commit()
        return types.Projection(projector=self.name)


class NoopProjector(Projector):
    """Projector that returns empty projection."""

    name = "projector-noop"
    input_domain = "inventory"

    @projects(StockUpdated)
    def project_stock(self, event: StockUpdated) -> None:
        return None


# =============================================================================
# Function-based Pattern: EventRouter with @event_handler
# =============================================================================


def build_audit_trail_router(engine: Engine) -> EventRouter:
    """Build function-based audit trail projector."""
    router = EventRouter("projector-audit-trail-fn").domain("inventory")

    @event_handler(StockUpdated)
    def handle_stock_updated(
        event: StockUpdated, root: bytes, correlation_id: str, destinations: list
    ) -> list:
        with engine.connect() as conn:
            conn.execute(
                insert(audit_trail).values(
                    event_type="StockUpdated",
                    aggregate_id=event.sku,
                    payload=f"quantity={event.quantity}",
                    occurred_at="now",
                )
            )
            conn.commit()
        return [types.Projection(projector="projector-audit-trail-fn")]

    router.on(handle_stock_updated)
    return router


def build_fulfillment_projector_router() -> EventRouter:
    """Projects fulfillment events (no DB, for comparison)."""
    router = EventRouter("projector-fulfillment-tracking").domain("fulfillment")

    @event_handler(ShipmentCreated)
    def handle_shipment_created(
        event: ShipmentCreated, root: bytes, correlation_id: str, destinations: list
    ) -> list:
        return [types.Projection(projector="projector-fulfillment-tracking")]

    router.on(handle_shipment_created)
    return router


# =============================================================================
# Tests for @projects decorator
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

            @projects(StockUpdated)
            def bad_method(self):
                pass

    def test_decorator_validates_missing_type_hint(self):
        with pytest.raises(TypeError, match="missing type hint"):

            @projects(StockUpdated)
            def bad_method(self, event):
                pass

    def test_decorator_validates_type_hint_mismatch(self):
        with pytest.raises(TypeError, match="doesn't match type hint"):

            @projects(StockUpdated)
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

            class BadProjector(Projector):
                input_domain = "inventory"

                @projects(StockUpdated)
                def handle(self, event: StockUpdated):
                    pass

    def test_missing_input_domain_raises(self):
        with pytest.raises(TypeError, match="must define 'input_domain'"):

            class BadProjector(Projector):
                name = "bad-projector"

                @projects(StockUpdated)
                def handle(self, event: StockUpdated):
                    pass

    def test_duplicate_handler_raises(self):
        with pytest.raises(TypeError, match="duplicate handler"):

            class BadProjector(Projector):
                name = "bad-projector"
                input_domain = "inventory"

                @projects(StockUpdated)
                def handle_one(self, event: StockUpdated):
                    pass

                @projects(StockUpdated)
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
# Tests for function-based pattern (router)
# =============================================================================


class TestFunctionBasedProjectorRouter:
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
        results = router.dispatch(source)

        assert len(results) == 1
        assert results[0].projector == "projector-audit-trail-fn"

        # Verify data in SQLite
        with engine.connect() as conn:
            result = conn.execute(select(audit_trail)).fetchall()
            assert len(result) == 1
            assert result[0].aggregate_id == "SKU-fn-123"


# =============================================================================
# Tests comparing both patterns produce equivalent output
# =============================================================================


class TestPatternEquivalence:
    """Verify OO and function-based patterns both write to SQLite correctly."""

    def test_same_db_state_for_stock_updated(self):
        # OO pattern
        oo_engine = create_test_db()
        oo_projector = AuditTrailProjector(oo_engine)

        event = StockUpdated(sku="SKU-eq", quantity=99)
        event_any = any_pb2.Any()
        event_any.Pack(event)
        oo_projector.dispatch(event_any)

        # Function pattern
        fn_engine = create_test_db()
        fn_router = build_audit_trail_router(fn_engine)
        source = types.EventBook(
            cover=types.Cover(domain="inventory"),
            pages=[types.EventPage(event=event_any)],
        )
        fn_router.dispatch(source)

        # Both produce same DB state
        with oo_engine.connect() as conn:
            oo_rows = conn.execute(select(audit_trail)).fetchall()

        with fn_engine.connect() as conn:
            fn_rows = conn.execute(select(audit_trail)).fetchall()

        assert len(oo_rows) == len(fn_rows) == 1
        assert oo_rows[0].event_type == fn_rows[0].event_type == "StockUpdated"
        assert oo_rows[0].aggregate_id == fn_rows[0].aggregate_id == "SKU-eq"
