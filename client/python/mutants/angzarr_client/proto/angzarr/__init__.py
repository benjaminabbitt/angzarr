"""Angzarr proto definitions."""

from .aggregate_pb2 import (
    SpeculatePmRequest,
    SpeculateProjectorRequest,
    SpeculateSagaRequest,
)
from .aggregate_pb2_grpc import (
    AggregateCoordinatorServiceStub,
    SpeculativeServiceStub,
)
from .process_manager_pb2 import ProcessManagerHandleResponse
from .query_pb2_grpc import EventQueryServiceStub
from .types_pb2 import (
    UUID,
    CommandBook,
    CommandPage,
    CommandResponse,
    Cover,
    DomainDivergence,
    DryRunRequest,
    Edition,
    EventBook,
    EventPage,
    Projection,
    Query,
    SagaResponse,
    SequenceRange,
    SequenceSet,
    Snapshot,
    SyncCommandBook,
    SyncEventBook,
    SyncMode,
    TemporalQuery,
)

__all__ = [
    # Types
    "UUID",
    "Cover",
    "Edition",
    "DomainDivergence",
    "EventPage",
    "EventBook",
    "Snapshot",
    "CommandPage",
    "CommandBook",
    "CommandResponse",
    "SyncCommandBook",
    "SyncEventBook",
    "Query",
    "SequenceRange",
    "SequenceSet",
    "TemporalQuery",
    "Projection",
    "SyncMode",
    # Aggregate
    "SpeculateProjectorRequest",
    "SpeculateSagaRequest",
    "SpeculatePmRequest",
    "DryRunRequest",
    # Stubs
    "AggregateCoordinatorServiceStub",
    "SpeculativeServiceStub",
    "EventQueryServiceStub",
    # Responses
    "SagaResponse",
    "ProcessManagerHandleResponse",
]
