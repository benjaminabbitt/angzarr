"""Pytest-bdd configuration and shared fixtures for client feature tests."""

import pytest

# Shared event store for cross-context access (similar to Go's SharedEventStore)
# This allows step definitions in one context to populate events that can be
# read by steps in another context (e.g., query_client -> domain_client)
SHARED_EVENT_STORE = {}


def clear_shared_event_store():
    """Clear the shared event store between tests."""
    SHARED_EVENT_STORE.clear()


# Import step definitions so pytest-bdd can find them
# Each module defines @given, @when, @then decorated functions
from tests.features.steps.aggregate_client import *  # noqa: F401, F403
from tests.features.steps.command_builder import *  # noqa: F401, F403
from tests.features.steps.compensation import *  # noqa: F401, F403
from tests.features.steps.connection import *  # noqa: F401, F403
from tests.features.steps.domain_client import *  # noqa: F401, F403
from tests.features.steps.error_handling import *  # noqa: F401, F403
from tests.features.steps.event_decoding import *  # noqa: F401, F403
from tests.features.steps.fact_flow import *  # noqa: F401, F403
from tests.features.steps.merge_strategy import *  # noqa: F401, F403
from tests.features.steps.query_builder import *  # noqa: F401, F403
from tests.features.steps.query_client import *  # noqa: F401, F403
from tests.features.steps.router import *  # noqa: F401, F403
from tests.features.steps.speculative_client import *  # noqa: F401, F403
from tests.features.steps.state_building import *  # noqa: F401, F403


@pytest.fixture(autouse=True)
def _clear_shared_store():
    """Clear shared event store before each test."""
    clear_shared_event_store()
    yield
    clear_shared_event_store()


@pytest.fixture
def context():
    """Shared test context for scenario state."""
    return {}


@pytest.fixture
def text():
    """Fixture for capturing docstring text from Gherkin steps.

    pytest-bdd passes docstrings (text between triple quotes) as the 'text' argument.
    This fixture provides a default empty string when no docstring is present.
    """
    return ""


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
