"""Angzarr Python client library for gRPC services."""

from importlib.metadata import version as _version

__version__ = _version("angzarr-client")

from .aggregate import Aggregate, applies, handles
from .aggregate_handler import AggregateHandler, run_aggregate_server
from .builder import CommandBuilder, QueryBuilder
from .client import (
    AggregateClient,
    Client,
    DomainClient,
    QueryClient,
    SpeculativeClient,
)
from .compensation import (
    CompensationContext,
    RejectionHandlerResponse,
    delegate_to_framework,
    emit_compensation_events,
    pm_delegate_to_framework,
    pm_emit_compensation_events,
)
from .errors import (
    ClientError,
    CommandRejectedError,
    ConnectionError,
    GRPCError,
    InvalidArgumentError,
    InvalidTimestampError,
    TransportError,
)
from .event_packing import new_event_book, new_event_book_multi, pack_event, pack_events
from .helpers import (
    correlation_id,
    destination_map,
    domain,
    edition,
    has_correlation_id,
    next_sequence,
    now,
    parse_timestamp,
    proto_to_uuid,
    root_id_hex,
    root_uuid,
    type_name_from_url,
    type_url,
    type_url_matches,
    uuid_to_proto,
)
from .identity import (
    INVENTORY_PRODUCT_NAMESPACE,
    cart_root,
    compute_root,
    customer_root,
    fulfillment_root,
    inventory_product_root,
    inventory_root,
    order_root,
    product_root,
    to_proto_bytes,
)
from .process_manager import ProcessManager
from .process_manager_handler import (
    PMHandleFunc,
    PMPrepareFunc,
    ProcessManagerHandler,
    run_process_manager_server,
)
from .projector import Projector
from .projector_handler import (
    ProjectorHandleFunc,
    ProjectorHandler,
    run_projector_server,
)
from .router import (
    ERRMSG_NO_COMMAND_PAGES,
    ERRMSG_UNKNOWN_COMMAND,
    CommandRouter,
    EventRouter,
    UpcasterRouter,
    command_handler,
    event_handler,
    prepares,
    projects,
    reacts_to,
    rejected,
    upcaster,
    validate_command_handler,
)
from .router import (
    next_sequence as router_next_sequence,
)
from .saga import Saga
from .saga_handler import ExecuteFunc, PrepareFunc, SagaHandler, run_saga_server
from .server import (
    cleanup_socket,
    configure_logging,
    create_server,
    get_transport_config,
    run_server,
)
from .state_builder import (
    SnapshotLoader,
    StateApplier,
    StateBuilder,
    StateFactory,
    StateRouter,
)
from .upcaster import Upcaster, upcasts
from .upcaster_handler import UpcasterHandleFunc, UpcasterHandler, run_upcaster_server
from .validation import (
    require_exists,
    require_non_negative,
    require_not_empty,
    require_not_exists,
    require_positive,
    require_status,
    require_status_not,
)
from .wrappers import (
    CommandBookW,
    CommandPageW,
    CommandResponseW,
    CoverW,
    EventBookW,
    EventPageW,
    QueryW,
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
    "destination_map",
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
