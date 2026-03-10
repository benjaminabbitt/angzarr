"""Angzarr proto definitions."""

from .command_handler_pb2 import (
    BusinessResponse,
    CommandResponse,
    RevocationResponse,
    SpeculateCommandHandlerRequest,
)
from .command_handler_pb2_grpc import CommandHandlerCoordinatorServiceStub
from .process_manager_pb2 import ProcessManagerHandleResponse, SpeculatePmRequest
from .process_manager_pb2_grpc import ProcessManagerCoordinatorServiceStub
from .projector_pb2 import SpeculateProjectorRequest
from .projector_pb2_grpc import ProjectorCoordinatorServiceStub
from .query_pb2_grpc import EventQueryServiceStub
from .saga_pb2 import (
    SagaHandleRequest,
    SagaResponse,
    SpeculateSagaRequest,
)
from .saga_pb2_grpc import SagaCoordinatorServiceStub
from .types_pb2 import (
    UUID,
    CommandBook,
    CommandPage,
    CommandRequest,
    ComponentDescriptor,
    ContextualCommand,
    Cover,
    DomainDivergence,
    Edition,
    EventBook,
    EventPage,
    EventRequest,
    GetDescriptorRequest,
    PageHeader,
    Projection,
    Query,
    SequenceRange,
    SequenceSet,
    Snapshot,
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
    "PageHeader",
    "Snapshot",
    "CommandPage",
    "CommandBook",
    "CommandRequest",
    "CommandResponse",
    "ComponentDescriptor",
    "EventRequest",
    "GetDescriptorRequest",
    "Query",
    "SequenceRange",
    "SequenceSet",
    "Target",
    "TemporalQuery",
    "Projection",
    "SyncMode",
    # Speculative
    "SpeculateCommandHandlerRequest",
    "SpeculateProjectorRequest",
    "SpeculateSagaRequest",
    "SpeculatePmRequest",
    # Stubs
    "CommandHandlerCoordinatorServiceStub",
    "SagaCoordinatorServiceStub",
    "ProjectorCoordinatorServiceStub",
    "ProcessManagerCoordinatorServiceStub",
    "EventQueryServiceStub",
    # Responses
    "BusinessResponse",
    "RevocationResponse",
    "SagaResponse",
    "ProcessManagerHandleResponse",
]
