"""Tests for client classes."""

import os
from unittest.mock import MagicMock, Mock, patch

import grpc
import pytest

from angzarr_client.client import (
    Client,
    CommandHandlerClient,
    DomainClient,
    QueryClient,
    SpeculativeClient,
)
from angzarr_client.errors import GRPCError
from angzarr_client.proto.angzarr import (
    CommandBook,
    CommandRequest,
    CommandResponse,
    EventBook,
    ProcessManagerHandleResponse,
    Projection,
    Query,
    SagaResponse,
    SpeculateCommandHandlerRequest,
    SpeculatePmRequest,
    SpeculateProjectorRequest,
    SpeculateSagaRequest,
)


class MockRpcError(grpc.RpcError):
    """Mock RpcError for testing.

    grpc.RpcError itself doesn't have code/details methods - those come
    from grpc.Call. Real gRPC errors inherit from both.
    """

    def __init__(self, code: grpc.StatusCode, details: str = ""):
        super().__init__()
        self._code = code
        self._details = details

    def code(self) -> grpc.StatusCode:
        return self._code

    def details(self) -> str:
        return self._details


class TestQueryClient:
    """Tests for QueryClient."""

    def _mock_channel(self) -> Mock:
        """Create a mock gRPC channel."""
        return Mock(spec=grpc.Channel)

    def test_init_creates_stub(self) -> None:
        """Constructor creates stub from channel."""
        channel = self._mock_channel()
        client = QueryClient(channel)
        assert client._channel is channel
        assert client._stub is not None

    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_connect(self, mock_channel: Mock) -> None:
        """connect creates client from endpoint."""
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = QueryClient.connect("localhost:9000")
        mock_channel.assert_called_once_with("localhost:9000")
        assert client is not None

    @patch.dict(os.environ, {"TEST_ENDPOINT": "env-host:9000"})
    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_from_env_uses_env_var(self, mock_channel: Mock) -> None:
        """from_env uses environment variable."""
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = QueryClient.from_env("TEST_ENDPOINT", "default:8000")
        mock_channel.assert_called_once_with("env-host:9000")

    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_from_env_uses_default(self, mock_channel: Mock) -> None:
        """from_env uses default when env var not set."""
        # Use a unique var name that won't exist, don't clear all env vars
        # (clearing all breaks mutmut which needs MUTANT_UNDER_TEST)
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = QueryClient.from_env(
            "QUERY_CLIENT_NONEXISTENT_VAR_12345", "default:8000"
        )
        mock_channel.assert_called_once_with("default:8000")

    def test_get_event_book_success(self) -> None:
        """get_event_book returns EventBook on success."""
        channel = self._mock_channel()
        client = QueryClient(channel)
        expected_book = EventBook()
        expected_book.next_sequence = 42
        client._stub.GetEventBook = Mock(return_value=expected_book)

        query = Query()
        result = client.get_event_book(query)

        client._stub.GetEventBook.assert_called_once_with(query)
        assert result.next_sequence == 42

    def test_get_event_book_raises_grpc_error(self) -> None:
        """get_event_book raises GRPCError on RpcError."""
        channel = self._mock_channel()
        client = QueryClient(channel)
        rpc_error = MockRpcError(grpc.StatusCode.NOT_FOUND, "not found")
        client._stub.GetEventBook = Mock(side_effect=rpc_error)

        query = Query()
        with pytest.raises(GRPCError) as exc_info:
            client.get_event_book(query)
        assert exc_info.value.is_not_found()

    def test_get_events_success(self) -> None:
        """get_events returns list of EventBooks."""
        channel = self._mock_channel()
        client = QueryClient(channel)
        book1 = EventBook()
        book1.next_sequence = 1
        book2 = EventBook()
        book2.next_sequence = 2
        client._stub.GetEvents = Mock(return_value=[book1, book2])

        query = Query()
        result = client.get_events(query)

        assert len(result) == 2
        assert result[0].next_sequence == 1
        assert result[1].next_sequence == 2

    def test_get_events_raises_grpc_error(self) -> None:
        """get_events raises GRPCError on RpcError."""
        channel = self._mock_channel()
        client = QueryClient(channel)
        rpc_error = MockRpcError(grpc.StatusCode.INTERNAL)
        client._stub.GetEvents = Mock(side_effect=rpc_error)

        query = Query()
        with pytest.raises(GRPCError):
            client.get_events(query)

    def test_close_closes_channel(self) -> None:
        """close closes the underlying channel."""
        channel = self._mock_channel()
        client = QueryClient(channel)
        client.close()
        channel.close.assert_called_once()


class TestCommandHandlerClient:
    """Tests for CommandHandlerClient."""

    def _mock_channel(self) -> Mock:
        """Create a mock gRPC channel."""
        return Mock(spec=grpc.Channel)

    def test_init_creates_stub(self) -> None:
        """Constructor creates stub from channel."""
        channel = self._mock_channel()
        client = CommandHandlerClient(channel)
        assert client._channel is channel
        assert client._stub is not None

    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_connect(self, mock_channel: Mock) -> None:
        """connect creates client from endpoint."""
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = CommandHandlerClient.connect("localhost:9000")
        mock_channel.assert_called_once_with("localhost:9000")

    @patch.dict(os.environ, {"AGG_ENDPOINT": "agg-host:9000"})
    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_from_env_uses_env_var(self, mock_channel: Mock) -> None:
        """from_env uses environment variable."""
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = CommandHandlerClient.from_env("AGG_ENDPOINT", "default:8000")
        mock_channel.assert_called_once_with("agg-host:9000")

    def test_handle_command_success(self) -> None:
        """handle_command returns CommandResponse on success."""
        channel = self._mock_channel()
        client = CommandHandlerClient(channel)
        expected_resp = CommandResponse()
        client._stub.HandleCommand = Mock(return_value=expected_resp)

        cmd = CommandRequest()
        result = client.handle_command(cmd)

        client._stub.HandleCommand.assert_called_once_with(cmd)
        assert result is not None

    def test_handle_command_raises_grpc_error(self) -> None:
        """handle_command raises GRPCError on RpcError."""
        channel = self._mock_channel()
        client = CommandHandlerClient(channel)
        rpc_error = MockRpcError(grpc.StatusCode.INTERNAL)
        client._stub.HandleCommand = Mock(side_effect=rpc_error)

        cmd = CommandRequest()
        with pytest.raises(GRPCError):
            client.handle_command(cmd)

    def test_handle_sync_speculative_success(self) -> None:
        """handle_sync_speculative returns CommandResponse on success."""
        channel = self._mock_channel()
        client = CommandHandlerClient(channel)
        expected_resp = CommandResponse()
        client._stub.HandleSyncSpeculative = Mock(return_value=expected_resp)

        request = SpeculateCommandHandlerRequest()
        result = client.handle_sync_speculative(request)

        client._stub.HandleSyncSpeculative.assert_called_once_with(request)
        assert result is not None

    def test_handle_sync_speculative_raises_grpc_error(self) -> None:
        """handle_sync_speculative raises GRPCError on RpcError."""
        channel = self._mock_channel()
        client = CommandHandlerClient(channel)
        rpc_error = MockRpcError(grpc.StatusCode.INVALID_ARGUMENT)
        client._stub.HandleSyncSpeculative = Mock(side_effect=rpc_error)

        request = SpeculateCommandHandlerRequest()
        with pytest.raises(GRPCError) as exc_info:
            client.handle_sync_speculative(request)
        assert exc_info.value.is_invalid_argument()

    def test_close_closes_channel(self) -> None:
        """close closes the underlying channel."""
        channel = self._mock_channel()
        client = CommandHandlerClient(channel)
        client.close()
        channel.close.assert_called_once()


class TestSpeculativeClient:
    """Tests for SpeculativeClient."""

    def _mock_channel(self) -> Mock:
        """Create a mock gRPC channel."""
        return Mock(spec=grpc.Channel)

    def test_init_creates_stubs(self) -> None:
        """Constructor creates stubs from channel."""
        channel = self._mock_channel()
        client = SpeculativeClient(channel)
        assert client._channel is channel
        assert client._command_handler_stub is not None
        assert client._saga_stub is not None
        assert client._projector_stub is not None
        assert client._pm_stub is not None

    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_connect(self, mock_channel: Mock) -> None:
        """connect creates client from endpoint."""
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = SpeculativeClient.connect("localhost:9000")
        mock_channel.assert_called_once_with("localhost:9000")

    @patch.dict(os.environ, {"SPEC_ENDPOINT": "spec-host:9000"})
    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_from_env_uses_env_var(self, mock_channel: Mock) -> None:
        """from_env uses environment variable."""
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = SpeculativeClient.from_env("SPEC_ENDPOINT", "default:8000")
        mock_channel.assert_called_once_with("spec-host:9000")

    def test_aggregate_success(self) -> None:
        """aggregate returns CommandResponse on success."""
        channel = self._mock_channel()
        client = SpeculativeClient(channel)
        expected_resp = CommandResponse()
        client._command_handler_stub.HandleSyncSpeculative = Mock(
            return_value=expected_resp
        )

        request = SpeculateCommandHandlerRequest()
        result = client.command_handler(request)

        client._command_handler_stub.HandleSyncSpeculative.assert_called_once_with(
            request
        )
        assert result is not None

    def test_aggregate_raises_grpc_error(self) -> None:
        """aggregate raises GRPCError on RpcError."""
        channel = self._mock_channel()
        client = SpeculativeClient(channel)
        rpc_error = MockRpcError(grpc.StatusCode.INTERNAL)
        client._command_handler_stub.HandleSyncSpeculative = Mock(side_effect=rpc_error)

        request = SpeculateCommandHandlerRequest()
        with pytest.raises(GRPCError):
            client.command_handler(request)

    def test_projector_success(self) -> None:
        """projector returns Projection on success."""
        channel = self._mock_channel()
        client = SpeculativeClient(channel)
        expected_proj = Projection()
        client._projector_stub.HandleSpeculative = Mock(return_value=expected_proj)

        request = SpeculateProjectorRequest()
        result = client.projector(request)

        client._projector_stub.HandleSpeculative.assert_called_once_with(request)
        assert result is not None

    def test_projector_raises_grpc_error(self) -> None:
        """projector raises GRPCError on RpcError."""
        channel = self._mock_channel()
        client = SpeculativeClient(channel)
        rpc_error = MockRpcError(grpc.StatusCode.INTERNAL)
        client._projector_stub.HandleSpeculative = Mock(side_effect=rpc_error)

        request = SpeculateProjectorRequest()
        with pytest.raises(GRPCError):
            client.projector(request)

    def test_saga_success(self) -> None:
        """saga returns SagaResponse on success."""
        channel = self._mock_channel()
        client = SpeculativeClient(channel)
        expected_resp = SagaResponse()
        client._saga_stub.ExecuteSpeculative = Mock(return_value=expected_resp)

        request = SpeculateSagaRequest()
        result = client.saga(request)

        client._saga_stub.ExecuteSpeculative.assert_called_once_with(request)
        assert result is not None

    def test_saga_raises_grpc_error(self) -> None:
        """saga raises GRPCError on RpcError."""
        channel = self._mock_channel()
        client = SpeculativeClient(channel)
        rpc_error = MockRpcError(grpc.StatusCode.INTERNAL)
        client._saga_stub.ExecuteSpeculative = Mock(side_effect=rpc_error)

        request = SpeculateSagaRequest()
        with pytest.raises(GRPCError):
            client.saga(request)

    def test_process_manager_success(self) -> None:
        """process_manager returns ProcessManagerHandleResponse on success."""
        channel = self._mock_channel()
        client = SpeculativeClient(channel)
        expected_resp = ProcessManagerHandleResponse()
        client._pm_stub.HandleSpeculative = Mock(return_value=expected_resp)

        request = SpeculatePmRequest()
        result = client.process_manager(request)

        client._pm_stub.HandleSpeculative.assert_called_once_with(request)
        assert result is not None

    def test_process_manager_raises_grpc_error(self) -> None:
        """process_manager raises GRPCError on RpcError."""
        channel = self._mock_channel()
        client = SpeculativeClient(channel)
        rpc_error = MockRpcError(grpc.StatusCode.INTERNAL)
        client._pm_stub.HandleSpeculative = Mock(side_effect=rpc_error)

        request = SpeculatePmRequest()
        with pytest.raises(GRPCError):
            client.process_manager(request)

    def test_close_closes_channel(self) -> None:
        """close closes the underlying channel."""
        channel = self._mock_channel()
        client = SpeculativeClient(channel)
        client.close()
        channel.close.assert_called_once()


class TestDomainClient:
    """Tests for DomainClient."""

    def _mock_channel(self) -> Mock:
        """Create a mock gRPC channel."""
        return Mock(spec=grpc.Channel)

    def test_init_creates_sub_clients(self) -> None:
        """Constructor creates aggregate and query clients."""
        channel = self._mock_channel()
        client = DomainClient(channel)
        assert client._channel is channel
        assert client.command_handler is not None
        assert client.query is not None
        assert isinstance(client.command_handler, CommandHandlerClient)
        assert isinstance(client.query, QueryClient)

    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_connect(self, mock_channel: Mock) -> None:
        """connect creates client from endpoint."""
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = DomainClient.connect("localhost:9000")
        mock_channel.assert_called_once_with("localhost:9000")

    @patch.dict(os.environ, {"DOMAIN_ENDPOINT": "domain-host:9000"})
    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_from_env_uses_env_var(self, mock_channel: Mock) -> None:
        """from_env uses environment variable."""
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = DomainClient.from_env("DOMAIN_ENDPOINT", "default:8000")
        mock_channel.assert_called_once_with("domain-host:9000")

    def test_execute_delegates_to_command_handler(self) -> None:
        """execute builds CommandRequest and delegates to command_handler.handle_command."""
        channel = self._mock_channel()
        client = DomainClient(channel)
        expected_resp = CommandResponse()
        expected_resp.events.next_sequence = 10
        client.command_handler._stub.HandleCommand = Mock(return_value=expected_resp)

        cmd = CommandBook()
        result = client.execute(cmd)

        assert result.events.next_sequence == 10

    def test_close_closes_channel(self) -> None:
        """close closes the underlying channel."""
        channel = self._mock_channel()
        client = DomainClient(channel)
        client.close()
        channel.close.assert_called_once()


class TestClient:
    """Tests for Client (combined client)."""

    def _mock_channel(self) -> Mock:
        """Create a mock gRPC channel."""
        return Mock(spec=grpc.Channel)

    def test_init_creates_all_sub_clients(self) -> None:
        """Constructor creates all sub-clients."""
        channel = self._mock_channel()
        client = Client(channel)
        assert client._channel is channel
        assert client.command_handler is not None
        assert client.query is not None
        assert client.speculative is not None
        assert isinstance(client.command_handler, CommandHandlerClient)
        assert isinstance(client.query, QueryClient)
        assert isinstance(client.speculative, SpeculativeClient)

    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_connect(self, mock_channel: Mock) -> None:
        """connect creates client from endpoint."""
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = Client.connect("localhost:9000")
        mock_channel.assert_called_once_with("localhost:9000")

    @patch.dict(os.environ, {"FULL_ENDPOINT": "full-host:9000"})
    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_from_env_uses_env_var(self, mock_channel: Mock) -> None:
        """from_env uses environment variable."""
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = Client.from_env("FULL_ENDPOINT", "default:8000")
        mock_channel.assert_called_once_with("full-host:9000")

    @patch("angzarr_client.client.grpc.insecure_channel")
    def test_from_env_uses_default(self, mock_channel: Mock) -> None:
        """from_env uses default when env var not set."""
        # Use a unique var name that won't exist, don't clear all env vars
        # (clearing all breaks mutmut which needs MUTANT_UNDER_TEST)
        mock_channel.return_value = Mock(spec=grpc.Channel)
        client = Client.from_env("CLIENT_NONEXISTENT_VAR_12345", "default:8000")
        mock_channel.assert_called_once_with("default:8000")

    def test_close_closes_channel(self) -> None:
        """close closes the underlying channel."""
        channel = self._mock_channel()
        client = Client(channel)
        client.close()
        channel.close.assert_called_once()
