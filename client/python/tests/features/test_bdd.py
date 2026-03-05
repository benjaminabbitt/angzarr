"""BDD test file that imports step definitions and registers scenarios.

pytest-bdd requires scenarios() to be called in a test module (test_*.py).
This file imports step definitions and then calls scenarios() to register
all feature file scenarios.
"""

from pytest_bdd import scenarios

# Import step definition modules to register step handlers
from tests.features.steps import (
    aggregate_client,  # noqa: F401
    command_builder,  # noqa: F401
    compensation,  # noqa: F401
    connection,  # noqa: F401
    domain_client,  # noqa: F401
    error_handling,  # noqa: F401
    event_decoding,  # noqa: F401
    fact_flow,  # noqa: F401
    merge_strategy,  # noqa: F401
    query_builder,  # noqa: F401
    query_client,  # noqa: F401
    router,  # noqa: F401
    speculative_client,  # noqa: F401
    state_building,  # noqa: F401
)

# Register all feature scenarios - pytest-bdd creates test functions from these
scenarios("aggregate_client.feature")
scenarios("command_builder.feature")
scenarios("compensation.feature")
scenarios("connection.feature")
scenarios("domain-client.feature")
scenarios("error_handling.feature")
scenarios("event_decoding.feature")
scenarios("fact_flow.feature")
scenarios("merge_strategy.feature")
scenarios("query_builder.feature")
scenarios("query_client.feature")
scenarios("router.feature")
scenarios("speculative_client.feature")
scenarios("state_building.feature")
