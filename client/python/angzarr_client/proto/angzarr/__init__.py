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
    SyncCommandBook,
    SyncEventBook,
    Query,
    SequenceRange,
    SequenceSet,
    TemporalQuery,
    Projection,
    SyncMode,
    GetDescriptorRequest,
    ComponentDescriptor,
    Target,
    ContextualCommand,
)
from .aggregate_pb2 import (
    CommandResponse,
    SpeculateAggregateRequest,
    BusinessResponse,
)
from .aggregate_pb2_grpc import AggregateCoordinatorServiceStub
from .saga_pb2 import (
    SagaResponse,
    SpeculateSagaRequest,
    SagaExecuteRequest,
    SagaPrepareRequest,
    SagaPrepareResponse,
)
from .saga_pb2_grpc import SagaCoordinatorServiceStub
from .projector_pb2 import SpeculateProjectorRequest
from .projector_pb2_grpc import ProjectorCoordinatorServiceStub
from .process_manager_pb2 import ProcessManagerHandleResponse, SpeculatePmRequest
from .process_manager_pb2_grpc import ProcessManagerCoordinatorServiceStub
from .query_pb2_grpc import EventQueryServiceStub

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
    # Speculative
    "SpeculateAggregateRequest",
    "SpeculateProjectorRequest",
    "SpeculateSagaRequest",
    "SpeculatePmRequest",
    # Stubs
    "AggregateCoordinatorServiceStub",
    "SagaCoordinatorServiceStub",
    "ProjectorCoordinatorServiceStub",
    "ProcessManagerCoordinatorServiceStub",
    "EventQueryServiceStub",
    # Responses
    "SagaResponse",
    "ProcessManagerHandleResponse",
]
