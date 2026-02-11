"""Angzarr proto definitions."""

from .types_pb2 import (
    UUID,
    Cover,
    Edition,
    DomainDivergence,
    EventPage,
    EventBook,
    Snapshot,
    CommandPage,
    CommandBook,
    CommandResponse,
    SyncCommandBook,
    SyncEventBook,
    Query,
    SequenceRange,
    SequenceSet,
    TemporalQuery,
    Projection,
    SyncMode,
)
from .aggregate_pb2 import (
    SpeculateProjectorRequest,
    SpeculateSagaRequest,
    SpeculatePmRequest,
    DryRunRequest,
)
from .aggregate_pb2_grpc import (
    AggregateCoordinatorServiceStub,
    SpeculativeServiceStub,
)
from .query_pb2_grpc import EventQueryServiceStub
from .saga_pb2 import SagaResponse
from .process_manager_pb2 import ProcessManagerHandleResponse

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
