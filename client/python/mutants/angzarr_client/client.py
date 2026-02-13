"""Client implementations for Angzarr gRPC services."""

import os
from typing import Optional

import grpc

from .proto.angzarr import (
    AggregateCoordinatorServiceStub,
    EventQueryServiceStub,
    SpeculativeServiceStub,
    CommandBook,
    CommandResponse,
    SyncCommandBook,
    DryRunRequest,
    EventBook,
    Query,
    Projection,
    SagaResponse,
    ProcessManagerHandleResponse,
    SpeculateProjectorRequest,
    SpeculateSagaRequest,
    SpeculatePmRequest,
)
from .errors import GRPCError, TransportError
from inspect import signature as _mutmut_signature
from typing import Annotated
from typing import Callable
from typing import ClassVar


MutantDict = Annotated[dict[str, Callable], "Mutant"]


def _mutmut_trampoline(orig, mutants, call_args, call_kwargs, self_arg = None):
    """Forward call to original or mutated function, depending on the environment"""
    import os
    mutant_under_test = os.environ['MUTANT_UNDER_TEST']
    if mutant_under_test == 'fail':
        from mutmut.__main__ import MutmutProgrammaticFailException
        raise MutmutProgrammaticFailException('Failed programmatically')      
    elif mutant_under_test == 'stats':
        from mutmut.__main__ import record_trampoline_hit
        record_trampoline_hit(orig.__module__ + '.' + orig.__name__)
        result = orig(*call_args, **call_kwargs)
        return result
    prefix = orig.__module__ + '.' + orig.__name__ + '__mutmut_'
    if not mutant_under_test.startswith(prefix):
        result = orig(*call_args, **call_kwargs)
        return result
    mutant_name = mutant_under_test.rpartition('.')[-1]
    if self_arg is not None:
        # call to a class method where self is not bound
        result = mutants[mutant_name](self_arg, *call_args, **call_kwargs)
    else:
        result = mutants[mutant_name](*call_args, **call_kwargs)
    return result


class QueryClient:
    """Client for the EventQueryService."""

    def xǁQueryClientǁ__init____mutmut_orig(self, channel: grpc.Channel):
        self._stub = EventQueryServiceStub(channel)
        self._channel = channel

    def xǁQueryClientǁ__init____mutmut_1(self, channel: grpc.Channel):
        self._stub = None
        self._channel = channel

    def xǁQueryClientǁ__init____mutmut_2(self, channel: grpc.Channel):
        self._stub = EventQueryServiceStub(None)
        self._channel = channel

    def xǁQueryClientǁ__init____mutmut_3(self, channel: grpc.Channel):
        self._stub = EventQueryServiceStub(channel)
        self._channel = None
    
    xǁQueryClientǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁQueryClientǁ__init____mutmut_1': xǁQueryClientǁ__init____mutmut_1, 
        'xǁQueryClientǁ__init____mutmut_2': xǁQueryClientǁ__init____mutmut_2, 
        'xǁQueryClientǁ__init____mutmut_3': xǁQueryClientǁ__init____mutmut_3
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁQueryClientǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁQueryClientǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁQueryClientǁ__init____mutmut_orig)
    xǁQueryClientǁ__init____mutmut_orig.__name__ = 'xǁQueryClientǁ__init__'

    @classmethod
    def connect(cls, endpoint: str) -> "QueryClient":
        """Connect to an event query service at the given endpoint."""
        channel = grpc.insecure_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "QueryClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def xǁQueryClientǁget_event_book__mutmut_orig(self, query: Query) -> EventBook:
        """Retrieve a single EventBook for the query."""
        try:
            return self._stub.GetEventBook(query)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁQueryClientǁget_event_book__mutmut_1(self, query: Query) -> EventBook:
        """Retrieve a single EventBook for the query."""
        try:
            return self._stub.GetEventBook(None)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁQueryClientǁget_event_book__mutmut_2(self, query: Query) -> EventBook:
        """Retrieve a single EventBook for the query."""
        try:
            return self._stub.GetEventBook(query)
        except grpc.RpcError as e:
            raise GRPCError(None) from e
    
    xǁQueryClientǁget_event_book__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁQueryClientǁget_event_book__mutmut_1': xǁQueryClientǁget_event_book__mutmut_1, 
        'xǁQueryClientǁget_event_book__mutmut_2': xǁQueryClientǁget_event_book__mutmut_2
    }
    
    def get_event_book(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁQueryClientǁget_event_book__mutmut_orig"), object.__getattribute__(self, "xǁQueryClientǁget_event_book__mutmut_mutants"), args, kwargs, self)
        return result 
    
    get_event_book.__signature__ = _mutmut_signature(xǁQueryClientǁget_event_book__mutmut_orig)
    xǁQueryClientǁget_event_book__mutmut_orig.__name__ = 'xǁQueryClientǁget_event_book'

    def xǁQueryClientǁget_events__mutmut_orig(self, query: Query) -> list[EventBook]:
        """Retrieve all EventBooks matching the query."""
        try:
            return list(self._stub.GetEvents(query))
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁQueryClientǁget_events__mutmut_1(self, query: Query) -> list[EventBook]:
        """Retrieve all EventBooks matching the query."""
        try:
            return list(None)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁQueryClientǁget_events__mutmut_2(self, query: Query) -> list[EventBook]:
        """Retrieve all EventBooks matching the query."""
        try:
            return list(self._stub.GetEvents(None))
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁQueryClientǁget_events__mutmut_3(self, query: Query) -> list[EventBook]:
        """Retrieve all EventBooks matching the query."""
        try:
            return list(self._stub.GetEvents(query))
        except grpc.RpcError as e:
            raise GRPCError(None) from e
    
    xǁQueryClientǁget_events__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁQueryClientǁget_events__mutmut_1': xǁQueryClientǁget_events__mutmut_1, 
        'xǁQueryClientǁget_events__mutmut_2': xǁQueryClientǁget_events__mutmut_2, 
        'xǁQueryClientǁget_events__mutmut_3': xǁQueryClientǁget_events__mutmut_3
    }
    
    def get_events(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁQueryClientǁget_events__mutmut_orig"), object.__getattribute__(self, "xǁQueryClientǁget_events__mutmut_mutants"), args, kwargs, self)
        return result 
    
    get_events.__signature__ = _mutmut_signature(xǁQueryClientǁget_events__mutmut_orig)
    xǁQueryClientǁget_events__mutmut_orig.__name__ = 'xǁQueryClientǁget_events'

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class AggregateClient:
    """Client for the AggregateCoordinatorService."""

    def xǁAggregateClientǁ__init____mutmut_orig(self, channel: grpc.Channel):
        self._stub = AggregateCoordinatorServiceStub(channel)
        self._channel = channel

    def xǁAggregateClientǁ__init____mutmut_1(self, channel: grpc.Channel):
        self._stub = None
        self._channel = channel

    def xǁAggregateClientǁ__init____mutmut_2(self, channel: grpc.Channel):
        self._stub = AggregateCoordinatorServiceStub(None)
        self._channel = channel

    def xǁAggregateClientǁ__init____mutmut_3(self, channel: grpc.Channel):
        self._stub = AggregateCoordinatorServiceStub(channel)
        self._channel = None
    
    xǁAggregateClientǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁAggregateClientǁ__init____mutmut_1': xǁAggregateClientǁ__init____mutmut_1, 
        'xǁAggregateClientǁ__init____mutmut_2': xǁAggregateClientǁ__init____mutmut_2, 
        'xǁAggregateClientǁ__init____mutmut_3': xǁAggregateClientǁ__init____mutmut_3
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁAggregateClientǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁAggregateClientǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁAggregateClientǁ__init____mutmut_orig)
    xǁAggregateClientǁ__init____mutmut_orig.__name__ = 'xǁAggregateClientǁ__init__'

    @classmethod
    def connect(cls, endpoint: str) -> "AggregateClient":
        """Connect to an aggregate coordinator at the given endpoint."""
        channel = grpc.insecure_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "AggregateClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def xǁAggregateClientǁhandle__mutmut_orig(self, command: CommandBook) -> CommandResponse:
        """Execute a command asynchronously."""
        try:
            return self._stub.Handle(command)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁAggregateClientǁhandle__mutmut_1(self, command: CommandBook) -> CommandResponse:
        """Execute a command asynchronously."""
        try:
            return self._stub.Handle(None)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁAggregateClientǁhandle__mutmut_2(self, command: CommandBook) -> CommandResponse:
        """Execute a command asynchronously."""
        try:
            return self._stub.Handle(command)
        except grpc.RpcError as e:
            raise GRPCError(None) from e
    
    xǁAggregateClientǁhandle__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁAggregateClientǁhandle__mutmut_1': xǁAggregateClientǁhandle__mutmut_1, 
        'xǁAggregateClientǁhandle__mutmut_2': xǁAggregateClientǁhandle__mutmut_2
    }
    
    def handle(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁAggregateClientǁhandle__mutmut_orig"), object.__getattribute__(self, "xǁAggregateClientǁhandle__mutmut_mutants"), args, kwargs, self)
        return result 
    
    handle.__signature__ = _mutmut_signature(xǁAggregateClientǁhandle__mutmut_orig)
    xǁAggregateClientǁhandle__mutmut_orig.__name__ = 'xǁAggregateClientǁhandle'

    def xǁAggregateClientǁhandle_sync__mutmut_orig(self, command: SyncCommandBook) -> CommandResponse:
        """Execute a command synchronously with the specified sync mode."""
        try:
            return self._stub.HandleSync(command)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁAggregateClientǁhandle_sync__mutmut_1(self, command: SyncCommandBook) -> CommandResponse:
        """Execute a command synchronously with the specified sync mode."""
        try:
            return self._stub.HandleSync(None)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁAggregateClientǁhandle_sync__mutmut_2(self, command: SyncCommandBook) -> CommandResponse:
        """Execute a command synchronously with the specified sync mode."""
        try:
            return self._stub.HandleSync(command)
        except grpc.RpcError as e:
            raise GRPCError(None) from e
    
    xǁAggregateClientǁhandle_sync__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁAggregateClientǁhandle_sync__mutmut_1': xǁAggregateClientǁhandle_sync__mutmut_1, 
        'xǁAggregateClientǁhandle_sync__mutmut_2': xǁAggregateClientǁhandle_sync__mutmut_2
    }
    
    def handle_sync(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁAggregateClientǁhandle_sync__mutmut_orig"), object.__getattribute__(self, "xǁAggregateClientǁhandle_sync__mutmut_mutants"), args, kwargs, self)
        return result 
    
    handle_sync.__signature__ = _mutmut_signature(xǁAggregateClientǁhandle_sync__mutmut_orig)
    xǁAggregateClientǁhandle_sync__mutmut_orig.__name__ = 'xǁAggregateClientǁhandle_sync'

    def xǁAggregateClientǁdry_run_handle__mutmut_orig(self, request: DryRunRequest) -> CommandResponse:
        """Execute a command in dry-run mode (no persistence)."""
        try:
            return self._stub.DryRunHandle(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁAggregateClientǁdry_run_handle__mutmut_1(self, request: DryRunRequest) -> CommandResponse:
        """Execute a command in dry-run mode (no persistence)."""
        try:
            return self._stub.DryRunHandle(None)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁAggregateClientǁdry_run_handle__mutmut_2(self, request: DryRunRequest) -> CommandResponse:
        """Execute a command in dry-run mode (no persistence)."""
        try:
            return self._stub.DryRunHandle(request)
        except grpc.RpcError as e:
            raise GRPCError(None) from e
    
    xǁAggregateClientǁdry_run_handle__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁAggregateClientǁdry_run_handle__mutmut_1': xǁAggregateClientǁdry_run_handle__mutmut_1, 
        'xǁAggregateClientǁdry_run_handle__mutmut_2': xǁAggregateClientǁdry_run_handle__mutmut_2
    }
    
    def dry_run_handle(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁAggregateClientǁdry_run_handle__mutmut_orig"), object.__getattribute__(self, "xǁAggregateClientǁdry_run_handle__mutmut_mutants"), args, kwargs, self)
        return result 
    
    dry_run_handle.__signature__ = _mutmut_signature(xǁAggregateClientǁdry_run_handle__mutmut_orig)
    xǁAggregateClientǁdry_run_handle__mutmut_orig.__name__ = 'xǁAggregateClientǁdry_run_handle'

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class SpeculativeClient:
    """Client for the SpeculativeService."""

    def xǁSpeculativeClientǁ__init____mutmut_orig(self, channel: grpc.Channel):
        self._stub = SpeculativeServiceStub(channel)
        self._channel = channel

    def xǁSpeculativeClientǁ__init____mutmut_1(self, channel: grpc.Channel):
        self._stub = None
        self._channel = channel

    def xǁSpeculativeClientǁ__init____mutmut_2(self, channel: grpc.Channel):
        self._stub = SpeculativeServiceStub(None)
        self._channel = channel

    def xǁSpeculativeClientǁ__init____mutmut_3(self, channel: grpc.Channel):
        self._stub = SpeculativeServiceStub(channel)
        self._channel = None
    
    xǁSpeculativeClientǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁSpeculativeClientǁ__init____mutmut_1': xǁSpeculativeClientǁ__init____mutmut_1, 
        'xǁSpeculativeClientǁ__init____mutmut_2': xǁSpeculativeClientǁ__init____mutmut_2, 
        'xǁSpeculativeClientǁ__init____mutmut_3': xǁSpeculativeClientǁ__init____mutmut_3
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁSpeculativeClientǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁSpeculativeClientǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁSpeculativeClientǁ__init____mutmut_orig)
    xǁSpeculativeClientǁ__init____mutmut_orig.__name__ = 'xǁSpeculativeClientǁ__init__'

    @classmethod
    def connect(cls, endpoint: str) -> "SpeculativeClient":
        """Connect to a speculative service at the given endpoint."""
        channel = grpc.insecure_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "SpeculativeClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def xǁSpeculativeClientǁdry_run__mutmut_orig(self, request: DryRunRequest) -> CommandResponse:
        """Execute a command without persistence."""
        try:
            return self._stub.DryRunCommand(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁSpeculativeClientǁdry_run__mutmut_1(self, request: DryRunRequest) -> CommandResponse:
        """Execute a command without persistence."""
        try:
            return self._stub.DryRunCommand(None)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁSpeculativeClientǁdry_run__mutmut_2(self, request: DryRunRequest) -> CommandResponse:
        """Execute a command without persistence."""
        try:
            return self._stub.DryRunCommand(request)
        except grpc.RpcError as e:
            raise GRPCError(None) from e
    
    xǁSpeculativeClientǁdry_run__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁSpeculativeClientǁdry_run__mutmut_1': xǁSpeculativeClientǁdry_run__mutmut_1, 
        'xǁSpeculativeClientǁdry_run__mutmut_2': xǁSpeculativeClientǁdry_run__mutmut_2
    }
    
    def dry_run(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁSpeculativeClientǁdry_run__mutmut_orig"), object.__getattribute__(self, "xǁSpeculativeClientǁdry_run__mutmut_mutants"), args, kwargs, self)
        return result 
    
    dry_run.__signature__ = _mutmut_signature(xǁSpeculativeClientǁdry_run__mutmut_orig)
    xǁSpeculativeClientǁdry_run__mutmut_orig.__name__ = 'xǁSpeculativeClientǁdry_run'

    def xǁSpeculativeClientǁprojector__mutmut_orig(self, request: SpeculateProjectorRequest) -> Projection:
        """Speculatively execute a projector against events."""
        try:
            return self._stub.SpeculateProjector(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁSpeculativeClientǁprojector__mutmut_1(self, request: SpeculateProjectorRequest) -> Projection:
        """Speculatively execute a projector against events."""
        try:
            return self._stub.SpeculateProjector(None)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁSpeculativeClientǁprojector__mutmut_2(self, request: SpeculateProjectorRequest) -> Projection:
        """Speculatively execute a projector against events."""
        try:
            return self._stub.SpeculateProjector(request)
        except grpc.RpcError as e:
            raise GRPCError(None) from e
    
    xǁSpeculativeClientǁprojector__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁSpeculativeClientǁprojector__mutmut_1': xǁSpeculativeClientǁprojector__mutmut_1, 
        'xǁSpeculativeClientǁprojector__mutmut_2': xǁSpeculativeClientǁprojector__mutmut_2
    }
    
    def projector(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁSpeculativeClientǁprojector__mutmut_orig"), object.__getattribute__(self, "xǁSpeculativeClientǁprojector__mutmut_mutants"), args, kwargs, self)
        return result 
    
    projector.__signature__ = _mutmut_signature(xǁSpeculativeClientǁprojector__mutmut_orig)
    xǁSpeculativeClientǁprojector__mutmut_orig.__name__ = 'xǁSpeculativeClientǁprojector'

    def xǁSpeculativeClientǁsaga__mutmut_orig(self, request: SpeculateSagaRequest) -> SagaResponse:
        """Speculatively execute a saga against events."""
        try:
            return self._stub.SpeculateSaga(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁSpeculativeClientǁsaga__mutmut_1(self, request: SpeculateSagaRequest) -> SagaResponse:
        """Speculatively execute a saga against events."""
        try:
            return self._stub.SpeculateSaga(None)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁSpeculativeClientǁsaga__mutmut_2(self, request: SpeculateSagaRequest) -> SagaResponse:
        """Speculatively execute a saga against events."""
        try:
            return self._stub.SpeculateSaga(request)
        except grpc.RpcError as e:
            raise GRPCError(None) from e
    
    xǁSpeculativeClientǁsaga__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁSpeculativeClientǁsaga__mutmut_1': xǁSpeculativeClientǁsaga__mutmut_1, 
        'xǁSpeculativeClientǁsaga__mutmut_2': xǁSpeculativeClientǁsaga__mutmut_2
    }
    
    def saga(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁSpeculativeClientǁsaga__mutmut_orig"), object.__getattribute__(self, "xǁSpeculativeClientǁsaga__mutmut_mutants"), args, kwargs, self)
        return result 
    
    saga.__signature__ = _mutmut_signature(xǁSpeculativeClientǁsaga__mutmut_orig)
    xǁSpeculativeClientǁsaga__mutmut_orig.__name__ = 'xǁSpeculativeClientǁsaga'

    def xǁSpeculativeClientǁprocess_manager__mutmut_orig(
        self, request: SpeculatePmRequest
    ) -> ProcessManagerHandleResponse:
        """Speculatively execute a process manager."""
        try:
            return self._stub.SpeculateProcessManager(request)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁSpeculativeClientǁprocess_manager__mutmut_1(
        self, request: SpeculatePmRequest
    ) -> ProcessManagerHandleResponse:
        """Speculatively execute a process manager."""
        try:
            return self._stub.SpeculateProcessManager(None)
        except grpc.RpcError as e:
            raise GRPCError(e) from e

    def xǁSpeculativeClientǁprocess_manager__mutmut_2(
        self, request: SpeculatePmRequest
    ) -> ProcessManagerHandleResponse:
        """Speculatively execute a process manager."""
        try:
            return self._stub.SpeculateProcessManager(request)
        except grpc.RpcError as e:
            raise GRPCError(None) from e
    
    xǁSpeculativeClientǁprocess_manager__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁSpeculativeClientǁprocess_manager__mutmut_1': xǁSpeculativeClientǁprocess_manager__mutmut_1, 
        'xǁSpeculativeClientǁprocess_manager__mutmut_2': xǁSpeculativeClientǁprocess_manager__mutmut_2
    }
    
    def process_manager(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁSpeculativeClientǁprocess_manager__mutmut_orig"), object.__getattribute__(self, "xǁSpeculativeClientǁprocess_manager__mutmut_mutants"), args, kwargs, self)
        return result 
    
    process_manager.__signature__ = _mutmut_signature(xǁSpeculativeClientǁprocess_manager__mutmut_orig)
    xǁSpeculativeClientǁprocess_manager__mutmut_orig.__name__ = 'xǁSpeculativeClientǁprocess_manager'

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class DomainClient:
    """Combined client for aggregate and query operations on a single domain."""

    def xǁDomainClientǁ__init____mutmut_orig(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = QueryClient(channel)
        self._channel = channel

    def xǁDomainClientǁ__init____mutmut_1(self, channel: grpc.Channel):
        self.aggregate = None
        self.query = QueryClient(channel)
        self._channel = channel

    def xǁDomainClientǁ__init____mutmut_2(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(None)
        self.query = QueryClient(channel)
        self._channel = channel

    def xǁDomainClientǁ__init____mutmut_3(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = None
        self._channel = channel

    def xǁDomainClientǁ__init____mutmut_4(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = QueryClient(None)
        self._channel = channel

    def xǁDomainClientǁ__init____mutmut_5(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = QueryClient(channel)
        self._channel = None
    
    xǁDomainClientǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁDomainClientǁ__init____mutmut_1': xǁDomainClientǁ__init____mutmut_1, 
        'xǁDomainClientǁ__init____mutmut_2': xǁDomainClientǁ__init____mutmut_2, 
        'xǁDomainClientǁ__init____mutmut_3': xǁDomainClientǁ__init____mutmut_3, 
        'xǁDomainClientǁ__init____mutmut_4': xǁDomainClientǁ__init____mutmut_4, 
        'xǁDomainClientǁ__init____mutmut_5': xǁDomainClientǁ__init____mutmut_5
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁDomainClientǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁDomainClientǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁDomainClientǁ__init____mutmut_orig)
    xǁDomainClientǁ__init____mutmut_orig.__name__ = 'xǁDomainClientǁ__init__'

    @classmethod
    def connect(cls, endpoint: str) -> "DomainClient":
        """Connect to a domain's coordinator at the given endpoint."""
        channel = grpc.insecure_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "DomainClient":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def xǁDomainClientǁexecute__mutmut_orig(self, command: CommandBook) -> CommandResponse:
        """Execute a command (delegates to aggregate client)."""
        return self.aggregate.handle(command)

    def xǁDomainClientǁexecute__mutmut_1(self, command: CommandBook) -> CommandResponse:
        """Execute a command (delegates to aggregate client)."""
        return self.aggregate.handle(None)
    
    xǁDomainClientǁexecute__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁDomainClientǁexecute__mutmut_1': xǁDomainClientǁexecute__mutmut_1
    }
    
    def execute(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁDomainClientǁexecute__mutmut_orig"), object.__getattribute__(self, "xǁDomainClientǁexecute__mutmut_mutants"), args, kwargs, self)
        return result 
    
    execute.__signature__ = _mutmut_signature(xǁDomainClientǁexecute__mutmut_orig)
    xǁDomainClientǁexecute__mutmut_orig.__name__ = 'xǁDomainClientǁexecute'

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()


class Client:
    """Combined client for aggregate, query, and speculative operations."""

    def xǁClientǁ__init____mutmut_orig(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = QueryClient(channel)
        self.speculative = SpeculativeClient(channel)
        self._channel = channel

    def xǁClientǁ__init____mutmut_1(self, channel: grpc.Channel):
        self.aggregate = None
        self.query = QueryClient(channel)
        self.speculative = SpeculativeClient(channel)
        self._channel = channel

    def xǁClientǁ__init____mutmut_2(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(None)
        self.query = QueryClient(channel)
        self.speculative = SpeculativeClient(channel)
        self._channel = channel

    def xǁClientǁ__init____mutmut_3(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = None
        self.speculative = SpeculativeClient(channel)
        self._channel = channel

    def xǁClientǁ__init____mutmut_4(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = QueryClient(None)
        self.speculative = SpeculativeClient(channel)
        self._channel = channel

    def xǁClientǁ__init____mutmut_5(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = QueryClient(channel)
        self.speculative = None
        self._channel = channel

    def xǁClientǁ__init____mutmut_6(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = QueryClient(channel)
        self.speculative = SpeculativeClient(None)
        self._channel = channel

    def xǁClientǁ__init____mutmut_7(self, channel: grpc.Channel):
        self.aggregate = AggregateClient(channel)
        self.query = QueryClient(channel)
        self.speculative = SpeculativeClient(channel)
        self._channel = None
    
    xǁClientǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁClientǁ__init____mutmut_1': xǁClientǁ__init____mutmut_1, 
        'xǁClientǁ__init____mutmut_2': xǁClientǁ__init____mutmut_2, 
        'xǁClientǁ__init____mutmut_3': xǁClientǁ__init____mutmut_3, 
        'xǁClientǁ__init____mutmut_4': xǁClientǁ__init____mutmut_4, 
        'xǁClientǁ__init____mutmut_5': xǁClientǁ__init____mutmut_5, 
        'xǁClientǁ__init____mutmut_6': xǁClientǁ__init____mutmut_6, 
        'xǁClientǁ__init____mutmut_7': xǁClientǁ__init____mutmut_7
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁClientǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁClientǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁClientǁ__init____mutmut_orig)
    xǁClientǁ__init____mutmut_orig.__name__ = 'xǁClientǁ__init__'

    @classmethod
    def connect(cls, endpoint: str) -> "Client":
        """Connect to a server providing all services."""
        channel = grpc.insecure_channel(endpoint)
        return cls(channel)

    @classmethod
    def from_env(cls, env_var: str, default: str) -> "Client":
        """Connect using an environment variable with fallback."""
        endpoint = os.environ.get(env_var, default)
        return cls.connect(endpoint)

    def close(self) -> None:
        """Close the underlying channel."""
        self._channel.close()
