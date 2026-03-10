"""Angzarr Python client library for gRPC services."""

from importlib.metadata import version as _version

__version__ = _version("angzarr-client")

from .aggregate import CommandHandler, applies, handles
from .aggregate_handler import CommandHandlerGrpc, run_command_handler_server
from .builder import CommandBuilder, QueryBuilder
from .client import (
    Client,
    CommandHandlerClient,
    DomainClient,
    QueryClient,
    SpeculativeClient,
)
from .cloudevents import (
    CloudEvent,
    CloudEventsProjector,
    CloudEventsResponse,
    CloudEventsRouter,
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
from .handler_protocols import (
    CommandHandlerDomainHandler,
    ProcessManagerDomainHandler,
    ProcessManagerResponse,
    ProjectorDomainHandler,
)
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
    CommandHandlerRouter,
    FluentRouter,
    OORouter,
    ProcessManagerRouter,
    ProjectorRouter,
    Router,
    SingleFluentRouter,
    UpcasterRouter,
    command_handler,  # Decorator for functional command handlers
    output_domain,  # Output domain decorator for sagas/PM methods
    prepares,  # Prepare handler decorator for two-phase protocol
    rejected,
)
from .router import (
    domain as domain_class,  # Class decorator for input domain (@domain("name"))
)
from .router import (
    handles as event_handles,  # Unified decorator for event handlers (sagas/PMs/projectors)
)
from .router import (
    next_sequence as router_next_sequence,
)
from .saga import Saga
from .saga_context import SagaContext
from .saga_handler import HandleFunc, SagaHandler, run_saga_server
from .server import (
    cleanup_socket,
    configure_logging,
    create_server,
    get_transport_config,
    run_server,
)
from .state_builder import (
    CommandRouter,
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
    "CommandHandlerClient",
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
    "ERRMSG_UNKNOWN_COMMAND",
    "ERRMSG_NO_COMMAND_PAGES",
    "router_next_sequence",
    "domain_class",  # Class decorator for input domain (@domain("name"))
    "output_domain",  # Output domain decorator for sagas/PM methods (@output_domain("name"))
    "event_handles",  # Unified decorator for event handlers (import from module for component-specific)
    "prepares",  # Prepare handler decorator for two-phase protocol
    "rejected",
    "command_handler",  # Decorator for functional command handlers
    # Server
    "configure_logging",
    "get_transport_config",
    "create_server",
    "run_server",
    "cleanup_socket",
    # Handlers
    "CommandHandlerGrpc",
    "run_command_handler_server",
    "SagaHandler",
    "run_saga_server",
    "HandleFunc",
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
    "CommandRouter",
    # Component base classes
    "CommandHandler",
    "handles",
    "applies",
    "Saga",
    "ProcessManager",
    "Projector",
    "Upcaster",
    "upcasts",
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
    # Unified Router (core)
    "Router",
    # Fluent Routers (functional handlers)
    "FluentRouter",
    "SingleFluentRouter",
    # OO Routers (decorator handlers)
    "OORouter",
    # Component-Specific Routers
    "CommandHandlerRouter",
    "UpcasterRouter",
    "ProcessManagerRouter",
    "ProjectorRouter",
    # Handler Protocols
    "CommandHandlerDomainHandler",
    "ProcessManagerDomainHandler",
    "ProjectorDomainHandler",
    "ProcessManagerResponse",
    # CloudEvents
    "CloudEvent",
    "CloudEventsProjector",
    "CloudEventsRouter",
    "CloudEventsResponse",
    # Saga Context
    "SagaContext",
]
