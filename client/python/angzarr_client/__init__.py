"""Angzarr Python client library for gRPC services."""

from .client import (
    AggregateClient,
    QueryClient,
    SpeculativeClient,
    DomainClient,
    Client,
)
from .errors import (
    ClientError,
    ConnectionError,
    TransportError,
    GRPCError,
    InvalidArgumentError,
    InvalidTimestampError,
    CommandRejectedError,
)
from .helpers import (
    domain,
    correlation_id,
    has_correlation_id,
    root_uuid,
    root_id_hex,
    edition,
    next_sequence,
    uuid_to_proto,
    proto_to_uuid,
    type_url,
    type_name_from_url,
    type_url_matches,
    now,
    parse_timestamp,
)
from .builder import CommandBuilder, QueryBuilder
from .wrappers import (
    CoverW,
    EventBookW,
    CommandBookW,
    QueryW,
    EventPageW,
    CommandPageW,
    CommandResponseW,
)
from .router import (
    CommandRouter,
    EventRouter,
    Descriptor,
    TargetDesc,
    COMPONENT_AGGREGATE,
    COMPONENT_SAGA,
    ERRMSG_UNKNOWN_COMMAND,
    ERRMSG_NO_COMMAND_PAGES,
    next_sequence as router_next_sequence,
    command_handler,
    event_handler,
    validate_command_handler,
    prepares,
    reacts_to,
    projects,
    rejected,
)
from .server import (
    configure_logging,
    get_transport_config,
    create_server,
    run_server,
    cleanup_socket,
)
from .aggregate_handler import AggregateHandler, run_aggregate_server
from .saga_handler import SagaHandler, run_saga_server, PrepareFunc, ExecuteFunc
from .process_manager_handler import (
    ProcessManagerHandler,
    run_process_manager_server,
    PMPrepareFunc,
    PMHandleFunc,
)
from .projector_handler import (
    ProjectorHandler,
    run_projector_server,
    ProjectorHandleFunc,
)
from .validation import (
    require_exists,
    require_not_exists,
    require_positive,
    require_non_negative,
    require_not_empty,
    require_status,
    require_status_not,
)
from .identity import (
    INVENTORY_PRODUCT_NAMESPACE,
    compute_root,
    inventory_product_root,
    customer_root,
    product_root,
    order_root,
    inventory_root,
    cart_root,
    fulfillment_root,
    to_proto_bytes,
)
from .event_packing import pack_event, pack_events, new_event_book, new_event_book_multi
from .state_builder import StateBuilder, StateRouter, StateApplier, SnapshotLoader, StateFactory
from .aggregate import Aggregate, handles, applies
from .saga import Saga
from .process_manager import ProcessManager
from .projector import Projector
from .upcaster import Upcaster, upcasts
from .upcaster_handler import UpcasterHandler, run_upcaster_server, UpcasterHandleFunc
from .router import upcaster, UpcasterRouter
from .compensation import (
    CompensationContext,
    RejectionHandlerResponse,
    delegate_to_framework,
    emit_compensation_events,
    pm_delegate_to_framework,
    pm_emit_compensation_events,
)

__all__ = [
    # Clients
    "AggregateClient",
    "QueryClient",
    "SpeculativeClient",
    "DomainClient",
    "Client",
    # Errors
    "ClientError",
    "ConnectionError",
    "TransportError",
    "GRPCError",
    "InvalidArgumentError",
    "InvalidTimestampError",
    "CommandRejectedError",
    # Helpers
    "domain",
    "correlation_id",
    "has_correlation_id",
    "root_uuid",
    "root_id_hex",
    "edition",
    "next_sequence",
    "uuid_to_proto",
    "proto_to_uuid",
    "type_url",
    "type_name_from_url",
    "type_url_matches",
    "now",
    "parse_timestamp",
    # Builders
    "CommandBuilder",
    "QueryBuilder",
    # Wrappers
    "CoverW",
    "EventBookW",
    "CommandBookW",
    "QueryW",
    "EventPageW",
    "CommandPageW",
    "CommandResponseW",
    # Router
    "CommandRouter",
    "EventRouter",
    "Descriptor",
    "TargetDesc",
    "COMPONENT_AGGREGATE",
    "COMPONENT_SAGA",
    "ERRMSG_UNKNOWN_COMMAND",
    "ERRMSG_NO_COMMAND_PAGES",
    "router_next_sequence",
    "command_handler",
    "event_handler",
    "prepares",
    "reacts_to",
    "projects",
    "rejected",
    # Server
    "configure_logging",
    "get_transport_config",
    "create_server",
    "run_server",
    "cleanup_socket",
    # Handlers
    "AggregateHandler",
    "run_aggregate_server",
    "SagaHandler",
    "run_saga_server",
    "PrepareFunc",
    "ExecuteFunc",
    "ProcessManagerHandler",
    "run_process_manager_server",
    "PMPrepareFunc",
    "PMHandleFunc",
    "ProjectorHandler",
    "run_projector_server",
    "ProjectorHandleFunc",
    # Validation
    "require_exists",
    "require_not_exists",
    "require_positive",
    "require_non_negative",
    "require_not_empty",
    "require_status",
    "require_status_not",
    # Identity
    "INVENTORY_PRODUCT_NAMESPACE",
    "compute_root",
    "inventory_product_root",
    "customer_root",
    "product_root",
    "order_root",
    "inventory_root",
    "cart_root",
    "fulfillment_root",
    "to_proto_bytes",
    # Event packing
    "pack_event",
    "pack_events",
    "new_event_book",
    "new_event_book_multi",
    # State builder
    "StateBuilder",
    "StateRouter",
    "StateApplier",
    "SnapshotLoader",
    "StateFactory",
    # Component base classes
    "Aggregate",
    "handles",
    "applies",
    "Saga",
    "ProcessManager",
    "Projector",
    "Upcaster",
    "upcasts",
    "upcaster",
    "UpcasterRouter",
    "UpcasterHandler",
    "run_upcaster_server",
    "UpcasterHandleFunc",
    # Compensation
    "CompensationContext",
    "RejectionHandlerResponse",
    "delegate_to_framework",
    "emit_compensation_events",
    "pm_delegate_to_framework",
    "pm_emit_compensation_events",
]
