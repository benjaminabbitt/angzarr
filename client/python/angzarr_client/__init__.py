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
]
