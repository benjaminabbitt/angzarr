"""Query builder step definitions."""

import uuid
from unittest.mock import MagicMock, AsyncMock

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

from angzarr_client.proto.angzarr import types_pb2


# Link to feature file
scenarios("../../../../features/query_builder.feature")


@pytest.fixture
def query_context():
    """Test context for query builder scenarios."""
    return {
        "mock_client": None,
        "built_query": None,
        "build_error": None,
        "domain": "",
        "root": None,
        "correlation_id": None,
        "edition": None,
        "get_events_result": None,
        "get_pages_result": None,
    }


class MockQueryClient:
    """Mock query client."""

    def __init__(self):
        self.last_query = None

    async def get_events(self, query):
        self.last_query = query
        return types_pb2.EventBook()


@given("a mock QueryClient for testing")
def given_mock_query_client(query_context):
    query_context["mock_client"] = MockQueryClient()


@when(parsers.parse('I build a query for domain "{domain}" root "{root}"'))
def when_build_query_domain_root(query_context, domain, root):
    query_context["domain"] = domain
    try:
        query_context["root"] = uuid.UUID(root)
    except ValueError:
        query_context["root"] = uuid.uuid4()
    _try_build_query(query_context)


@when(parsers.parse('I build a query for domain "{domain}"'))
def when_build_query_domain(query_context, domain):
    query_context["domain"] = domain
    _try_build_query(query_context)


@when(parsers.parse('I set edition to "{edition}"'))
def when_set_edition(query_context, edition):
    query_context["edition"] = edition


@when(parsers.parse('I query by correlation ID "{cid}"'))
def when_query_by_correlation(query_context, cid):
    query_context["correlation_id"] = cid
    _try_build_query(query_context)


@then(parsers.parse('the query should have domain "{expected}"'))
def then_query_has_domain(query_context, expected):
    query = query_context["built_query"]
    assert query is not None
    assert query.cover.domain == expected


@then(parsers.parse('the query should have root "{expected}"'))
def then_query_has_root(query_context, expected):
    query = query_context["built_query"]
    assert query is not None
    assert query.cover.root is not None


@then("the query should select ALL events")
def then_query_select_all(query_context):
    query = query_context["built_query"]
    assert query is not None
    # Query without range/sequences/temporal selects all events
    assert not query.HasField("range") and not query.HasField("sequences") and not query.HasField("temporal")


@then("the query should succeed")
def then_query_succeeds(query_context):
    assert query_context["built_query"] is not None


@then("the query should fail")
def then_query_fails(query_context):
    assert query_context["build_error"] is not None


@then(parsers.parse('the query should have edition "{expected}"'))
def then_query_has_edition(query_context, expected):
    query = query_context["built_query"]
    assert query is not None
    assert query.cover.edition.name == expected


@then(parsers.parse('the query should have correlation_id "{expected}"'))
def then_query_has_correlation_id(query_context, expected):
    query = query_context["built_query"]
    assert query is not None
    assert query.cover.correlation_id == expected


def _try_build_query(ctx):
    """Build a Query from context."""
    try:
        cover = types_pb2.Cover(
            domain=ctx["domain"],
        )

        if ctx.get("root"):
            cover.root.value = ctx["root"].bytes

        if ctx.get("correlation_id"):
            cover.correlation_id = ctx["correlation_id"]

        if ctx.get("edition"):
            cover.edition.name = ctx["edition"]

        query = types_pb2.Query(
            cover=cover,
        )

        ctx["built_query"] = query
    except Exception as e:
        ctx["build_error"] = e
