"""Angzarr proto definitions."""

from .aggregate_pb2 import (
    BusinessResponse,
    CommandResponse,
    SpeculateAggregateRequest,
)
from .aggregate_pb2_grpc import AggregateCoordinatorServiceStub
from .process_manager_pb2 import ProcessManagerHandleResponse, SpeculatePmRequest
from .process_manager_pb2_grpc import ProcessManagerCoordinatorServiceStub
from .projector_pb2 import SpeculateProjectorRequest
from .projector_pb2_grpc import ProjectorCoordinatorServiceStub
from .query_pb2_grpc import EventQueryServiceStub
from .saga_pb2 import (
    SagaExecuteRequest,
    SagaPrepareRequest,
    SagaPrepareResponse,
    SagaResponse,
    SpeculateSagaRequest,
)
from .saga_pb2_grpc import SagaCoordinatorServiceStub
from .types_pb2 import (
    UUID,
    CommandBook,
    CommandPage,
    ComponentDescriptor,
    ContextualCommand,
    Cover,
    DomainDivergence,
    Edition,
    EventBook,
    EventPage,
    GetDescriptorRequest,
    Projection,
    Query,
    SequenceRange,
    SequenceSet,
    Snapshot,
    SyncCommandBook,
    SyncEventBook,
    SyncMode,
    Target,
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
    "ComponentDescriptor",
    "GetDescriptorRequest",
    "SyncCommandBook",
    "SyncEventBook",
    "Query",
    "SequenceRange",
    "SequenceSet",
    "Target",
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
