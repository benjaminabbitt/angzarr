"""Pytest-bdd configuration and shared fixtures for client feature tests."""

import pytest


@pytest.fixture
def context():
    """Shared test context for scenario state."""
    return {}


@pytest.fixture
def mock_gateway(context):
    """Mock gateway client that records commands."""
    from unittest.mock import AsyncMock, MagicMock

    gateway = MagicMock()
    gateway.execute = AsyncMock(return_value=MagicMock(events=None))
    gateway.last_command = None
    context["mock_gateway"] = gateway
    return gateway


@pytest.fixture
def mock_query_client(context):
    """Mock query client that records queries."""
    from unittest.mock import AsyncMock, MagicMock

    client = MagicMock()
    client.get_events = AsyncMock(return_value=MagicMock(pages=[]))
    client.last_query = None
    context["mock_query_client"] = client
    return client
