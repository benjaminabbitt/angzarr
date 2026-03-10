"""Client implementations for Angzarr gRPC services."""

import os
from enum import Enum

import grpc

from .errors import GRPCError
from .proto.angzarr import (
    CommandBook,
    CommandHandlerCoordinatorServiceStub,
    CommandRequest,
    CommandResponse,
    EventBook,
    EventQueryServiceStub,
    ProcessManagerCoordinatorServiceStub,
    ProcessManagerHandleResponse,
    Projection,
    ProjectorCoordinatorServiceStub,
    Query,
    SagaCoordinatorServiceStub,
    SagaResponse,
    SpeculateCommandHandlerRequest,
    SpeculatePmRequest,
    SpeculateProjectorRequest,
    SpeculateSagaRequest,
    SyncMode,
)


class TransportMode(Enum):
    """Transport mode for gRPC connections."""

    STANDALONE = "standalone"  # Unix Domain Sockets
    DISTRIBUTED = "distributed"  # TCP via K8s DNS


def resolve_ch_endpoint(
    domain: str,
    mode: TransportMode | None = None,
    *,
    uds_base: str = "/tmp/angzarr",
    namespace: str = "angzarr",
    port: int = 1310,
) -> str:
    """Resolve domain to command handler coordinator endpoint.

    Args:
        domain: The domain name (e.g., "player", "table", "hand")
        mode: Transport mode. If None, detected from ANGZARR_MODE env var.
        uds_base: Base path for Unix Domain Sockets (standalone mode)
        namespace: Kubernetes namespace (distributed mode)
        port: gRPC port (distributed mode)

    Returns:
        Endpoint string suitable for _create_channel:
        - Standalone: /tmp/angzarr/ch-player.sock
        - Distributed: ch-player.angzarr.svc:1310

    Environment Variables:
        ANGZARR_MODE: "standalone" or "distributed" (default: "distributed")
        ANGZARR_UDS_BASE: Override uds_base (default: /tmp/angzarr)
        ANGZARR_NAMESPACE: Override namespace (default: angzarr)
        ANGZARR_CH_PORT: Override port (default: 1310)
    """
    if mode is None:
        mode_str = os.environ.get("ANGZARR_MODE", "distributed")
        mode = TransportMode(mode_str)

    if mode == TransportMode.STANDALONE:
        base = os.environ.get("ANGZARR_UDS_BASE", uds_base)
        return f"{base}/ch-{domain}.sock"
    else:
        ns = os.environ.get("ANGZARR_NAMESPACE", namespace)
        p = int(os.environ.get("ANGZARR_CH_PORT", str(port)))
        return f"ch-{domain}.{ns}.svc:{p}"


def _create_channel(endpoint: str) -> grpc.Channel:
    """Create a gRPC channel for the given endpoint.

    Supports both TCP (host:port) and Unix Domain Sockets (file paths).
    UDS paths are detected by leading '/' or './' and converted to unix: URIs.
    Note: grpc-python uses unix:path for relative, unix:///path for absolute.

    KNOWN LIMITATION: grpc-python 1.57+ has a cross-language interoperability
    issue with tonic (Rust) servers over Unix Domain Sockets. Connections fail
    with "RST_STREAM with error code 1" (PROTOCOL_ERROR). This is tracked in:
    - https://github.com/hyperium/tonic/issues/826
    - https://github.com/hyperium/tonic/issues/742
    - https://github.com/grpc/grpc/issues/34760

    Workaround: Use TCP instead of UDS when Python clients connect to Rust
    (tonic) servers. UDS works fine for same-language communication.
    """
    if endpoint.startswith("./"):
        # Relative Unix domain socket path - use unix:path format
        return grpc.insecure_channel(f"unix:{endpoint}")
    elif endpoint.startswith("/"):
        # Absolute Unix domain socket path - use unix:///path format
        return grpc.insecure_channel(f"unix://{endpoint}")
    elif endpoint.startswith("unix:"):
        # Already in URI format
        return grpc.insecure_channel(endpoint)
    else:
        # TCP endpoint (host:port)
        return grpc.insecure_channel(endpoint)


class QueryClient:
    """Client for the EventQueryService."""

    def __init__(self, channel: grpc.Channel):
        self._stub = EventQueryServiceStub(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "QueryClient":
        """Connect to an event query service at the given endpoint."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "QueryClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def get_event_book(self, query: Query) -> EventBook:
        """Retrieve a single EventBook for the query."""
        try:
            return self._stub.GetEventBook(query)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def get_events(self, query: Query) -> list[EventBook]:
        """Retrieve all EventBooks matching the query."""
        try:
            return list(self._stub.GetEvents(query))
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class CommandHandlerClient:
    """Client for the CommandHandlerCoordinatorService."""

    def __init__(self, channel: grpc.Channel):
        self._stub = CommandHandlerCoordinatorServiceStub(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "CommandHandlerClient":
        """Connect to a command handler coordinator at the given endpoint."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "CommandHandlerClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def handle_command(self, request: CommandRequest) -> CommandResponse:
        """Execute a command with the specified sync mode."""
        try:
            return self._stub.HandleCommand(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def handle_sync_speculative(
        self, request: SpeculateCommandHandlerRequest
    ) -> CommandResponse:
        """Execute a command speculatively against temporal state (no persistence)."""
        try:
            return self._stub.HandleSyncSpeculative(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class SpeculativeClient:
    """Client for speculative operations across coordinator services.

    Speculative execution runs commands/events against temporal state without persistence.
    Each coordinator service now provides its own speculative method.
    """

    def __init__(self, channel: grpc.Channel):
        self._command_handler_stub = CommandHandlerCoordinatorServiceStub(channel)
        self._saga_stub = SagaCoordinatorServiceStub(channel)
        self._projector_stub = ProjectorCoordinatorServiceStub(channel)
        self._pm_stub = ProcessManagerCoordinatorServiceStub(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "SpeculativeClient":
        """Connect to coordinator services at the given endpoint."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "SpeculativeClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def command_handler(
        self, request: SpeculateCommandHandlerRequest
    ) -> CommandResponse:
        """Execute a command speculatively against temporal state."""
        try:
            return self._command_handler_stub.HandleSyncSpeculative(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def projector(self, request: SpeculateProjectorRequest) -> Projection:
        """Speculatively execute a projector against events."""
        try:
            return self._projector_stub.HandleSpeculative(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def saga(self, request: SpeculateSagaRequest) -> SagaResponse:
        """Speculatively execute a saga against events."""
        try:
            return self._saga_stub.ExecuteSpeculative(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def process_manager(
        self, request: SpeculatePmRequest
    ) -> ProcessManagerHandleResponse:
        """Speculatively execute a process manager."""
        try:
            return self._pm_stub.HandleSpeculative(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class DomainClient:
    """Combined client for command handler and query operations on a single domain."""

    def __init__(self, channel: grpc.Channel):
        self.command_handler = CommandHandlerClient(channel)
        self.query = QueryClient(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "DomainClient":
        """Connect to a domain's coordinator at the given endpoint."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def for_domain(
        cls, domain: str, mode: TransportMode | None = None
    ) -> "DomainClient":
        """Connect to a domain's command handler coordinator.

        Resolves the domain name to the appropriate endpoint based on transport mode.

        Args:
            domain: Domain name (e.g., "player", "table")
            mode: Transport mode (standalone=UDS, distributed=K8s DNS).
                  If None, detected from ANGZARR_MODE env var.

        Returns:
            DomainClient connected to the domain's command handler coordinator.

        Examples:
            # Auto-detect mode from ANGZARR_MODE env var
            player = DomainClient.for_domain("player")

            # Explicitly use standalone mode (Unix Domain Sockets)
            player = DomainClient.for_domain("player", TransportMode.STANDALONE)

            # Explicitly use distributed mode (K8s DNS)
            player = DomainClient.for_domain("player", TransportMode.DISTRIBUTED)
        """
        endpoint = resolve_ch_endpoint(domain, mode)
        return cls.connect(endpoint)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "DomainClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def execute(
        self,
        command: CommandBook,
        sync_mode: SyncMode = SyncMode.SYNC_MODE_ASYNC,
    ) -> CommandResponse:
        """Execute a command with the specified sync mode.

        Args:
            command: The command to execute.
            sync_mode: Execution mode (ASYNC, SIMPLE, or CASCADE).
                      Defaults to ASYNC for fire-and-forget behavior.
        """
        request = CommandRequest(command=command, sync_mode=sync_mode)
        return self.command_handler.handle_command(request)

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class Client:
    """Combined client for command handler, query, and speculative operations."""

    def __init__(self, channel: grpc.Channel):
        self.command_handler = CommandHandlerClient(channel)
        self.query = QueryClient(channel)
        self.speculative = SpeculativeClient(channel)
        self._channel = channel

    @classmethod
    def connect(cls, endpoint: str) -> "Client":
        """Connect to a server providing all services."""
        channel = _create_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "Client":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()
