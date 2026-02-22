"""Angzarr Python client library for gRPC services."""

from .builder import CommandBuilder, QueryBuilder
from .client import (
    AggregateClient,
    Client,
    DomainClient,
    QueryClient,
    SpeculativeClient,
)
from .errors import (
    ClientError,
    ConnectionError,
    GRPCError,
    InvalidArgumentError,
    InvalidTimestampError,
    TransportError,
)
from .helpers import (
    correlation_id,
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
]
