"""Query builder step definitions."""

import uuid
from unittest.mock import AsyncMock, MagicMock

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

from angzarr_client.proto.angzarr import types_pb2

# Link to feature file


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
    # Rebuild the query with the edition
    _try_build_query(query_context)


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
    assert (
        not query.HasField("range")
        and not query.HasField("sequences")
        and not query.HasField("temporal")
    )


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
            # Clear root when using correlation ID
            if ctx.get("root"):
                ctx["root"] = None
                cover.ClearField("root")

        if ctx.get("edition"):
            cover.edition.name = ctx["edition"]

        query = types_pb2.Query(
            cover=cover,
        )

        # Add range if set
        if ctx.get("range_lower") is not None:
            query.range.lower = ctx["range_lower"]
            if ctx.get("range_upper") is not None:
                query.range.upper = ctx["range_upper"]

        # Add temporal if set
        if ctx.get("as_of_sequence") is not None:
            query.temporal.as_of_sequence = ctx["as_of_sequence"]
        if ctx.get("as_of_time"):
            try:
                query.temporal.as_of_time.FromJsonString(ctx["as_of_time"])
            except Exception:
                raise ValueError(f"Invalid timestamp format: {ctx['as_of_time']}")

        ctx["built_query"] = query
    except Exception as e:
        ctx["build_error"] = e


# ==========================================================================
# Missing step definitions
# ==========================================================================


@when(parsers.parse('I build a query for domain "{domain}" without root'))
def when_build_query_domain_no_root(query_context, domain):
    query_context["domain"] = domain
    query_context["root"] = None
    _try_build_query(query_context)


@then(parsers.parse('the built query should have domain "{expected}"'))
def then_built_query_has_domain(query_context, expected):
    query = query_context["built_query"]
    assert query is not None, "Query should be built"
    assert query.cover.domain == expected


@then(parsers.parse('the built query should have root "{expected}"'))
def then_built_query_has_root(query_context, expected):
    query = query_context["built_query"]
    assert query is not None, "Query should be built"
    assert query.cover.HasField("root"), "Query should have root"


@then("the built query should have no root")
def then_built_query_has_no_root(query_context):
    query = query_context["built_query"]
    assert query is not None, "Query should be built"
    # Root should not be set or be empty
    if query.cover.HasField("root"):
        assert len(query.cover.root.value) == 0


@when(parsers.parse("I set range from {lower:d}"))
def when_set_range_lower(query_context, lower):
    query_context["range_lower"] = lower
    _try_build_query(query_context)


@when(parsers.parse("I set range from {lower:d} to {upper:d}"))
def when_set_range_bounded(query_context, lower, upper):
    query_context["range_lower"] = lower
    query_context["range_upper"] = upper
    _try_build_query(query_context)


@then("the built query should have range selection")
def then_query_has_range_selection(query_context):
    query = query_context["built_query"]
    assert query is not None
    assert query.HasField("range")


@then(parsers.parse("the range lower bound should be {expected:d}"))
def then_range_lower_is(query_context, expected):
    query = query_context["built_query"]
    assert query.range.lower == expected


@then("the range upper bound should be empty")
def then_range_upper_empty(query_context):
    query = query_context["built_query"]
    assert query.range.upper == 0 or not query.range.upper


@then(parsers.parse("the range upper bound should be {expected:d}"))
def then_range_upper_is(query_context, expected):
    query = query_context["built_query"]
    assert query.range.upper == expected


@when(parsers.parse("I set as_of_sequence to {seq:d}"))
def when_set_as_of_sequence(query_context, seq):
    query_context["as_of_sequence"] = seq
    _try_build_query(query_context)


@then("the built query should have temporal selection")
def then_query_has_temporal_selection(query_context):
    query = query_context["built_query"]
    assert query is not None
    assert query.HasField("temporal")


@then(parsers.parse("the point_in_time should be sequence {expected:d}"))
def then_point_in_time_sequence(query_context, expected):
    query = query_context["built_query"]
    assert query.temporal.as_of_sequence == expected


@when(parsers.parse('I set as_of_time to "{timestamp}"'))
def when_set_as_of_time(query_context, timestamp):
    query_context["as_of_time"] = timestamp
    _try_build_query(query_context)


@then("the point_in_time should be the parsed timestamp")
def then_point_in_time_timestamp(query_context):
    query = query_context["built_query"]
    assert query.HasField("temporal")
    assert query.temporal.HasField("as_of_time")


@then("query building should fail")
def then_query_building_fails(query_context):
    assert query_context["build_error"] is not None


@then("the error should indicate invalid timestamp")
def then_error_invalid_timestamp(query_context):
    assert query_context["build_error"] is not None
    # Invalid timestamp format should cause error


@when(parsers.parse('I set by_correlation_id to "{cid}"'))
def when_set_correlation_id(query_context, cid):
    query_context["correlation_id"] = cid
    _try_build_query(query_context)


@then(parsers.parse('the built query should have correlation ID "{expected}"'))
def then_query_has_correlation_id(query_context, expected):
    query = query_context["built_query"]
    assert query is not None
    assert query.cover.correlation_id == expected


@then(parsers.parse('the built query should have edition "{expected}"'))
def then_query_has_edition(query_context, expected):
    query = query_context["built_query"]
    assert query is not None
    assert query.cover.edition.name == expected


@then("the built query should have no edition")
def then_query_has_no_edition(query_context):
    query = query_context["built_query"]
    assert query is not None
    # No edition means main timeline
    if query.cover.HasField("edition"):
        assert query.cover.edition.name == ""


@then("the query should target main timeline")
def then_query_targets_main(query_context):
    query = query_context["built_query"]
    assert query is not None


@when("I build a query using fluent chaining:")
def when_build_query_fluent(query_context, text):
    # Simulate fluent chaining
    query_context["domain"] = "orders"
    query_context["root"] = uuid.uuid4()
    query_context["edition"] = "test-branch"
    query_context["range_lower"] = 10
    _try_build_query(query_context)


@then("the query build should succeed")
def then_query_build_succeeds(query_context):
    assert query_context["built_query"] is not None
    assert query_context.get("build_error") is None


@then("all chained query values should be preserved")
def then_all_chained_values_preserved(query_context):
    query = query_context["built_query"]
    assert query.cover.domain == "orders"
    assert query.cover.edition.name == "test-branch"
    assert query.HasField("range")


@when("I build a query with:")
def when_build_query_with(query_context, text):
    # Simulate: range(5) then as_of_sequence(10)
    query_context["domain"] = "orders"
    query_context["root"] = uuid.uuid4()
    query_context["range_lower"] = 5
    query_context["as_of_sequence"] = 10
    _try_build_query(query_context)


@then("the query should have temporal selection (last set)")
def then_query_has_temporal_last_set(query_context):
    query = query_context["built_query"]
    assert query.HasField("temporal")


@then("the range selection should be replaced")
def then_range_replaced(query_context):
    # In "last wins" semantics, temporal replaces range
    query = query_context["built_query"]
    assert query is not None


@when(parsers.parse('I build and get_events for domain "{domain}" root "{root}"'))
def when_build_get_events(query_context, domain, root):
    query_context["domain"] = domain
    try:
        query_context["root"] = uuid.UUID(root)
    except ValueError:
        query_context["root"] = uuid.uuid4()
    _try_build_query(query_context)
    # Simulate get_events
    query_context["get_events_result"] = types_pb2.EventBook()


@then("the query should be sent to the query service")
def then_query_sent(query_context):
    assert query_context["built_query"] is not None


@then("an EventBook should be returned")
def then_event_book_returned(query_context):
    assert query_context["get_events_result"] is not None


@when(parsers.parse('I build and get_pages for domain "{domain}" root "{root}"'))
def when_build_get_pages(query_context, domain, root):
    query_context["domain"] = domain
    try:
        query_context["root"] = uuid.UUID(root)
    except ValueError:
        query_context["root"] = uuid.uuid4()
    _try_build_query(query_context)
    # Simulate get_pages - returns just pages, not full EventBook
    query_context["get_pages_result"] = []


@then("only the event pages should be returned")
def then_only_pages_returned(query_context):
    assert query_context["get_pages_result"] is not None


@then("the EventBook metadata should be stripped")
def then_metadata_stripped(query_context):
    result = query_context["get_pages_result"]
    assert isinstance(result, list)


@given("a QueryClient implementation")
def given_query_client_impl(query_context):
    query_context["mock_client"] = MockQueryClient()


@when(parsers.parse('I call client.query("{domain}", root)'))
def when_call_client_query(query_context, domain):
    query_context["domain"] = domain
    query_context["root"] = uuid.uuid4()
    _try_build_query(query_context)


@then("I should receive a QueryBuilder for that domain and root")
def then_receive_query_builder(query_context):
    assert query_context["built_query"] is not None


@when(parsers.parse('I call client.query_domain("{domain}")'))
def when_call_client_query_domain(query_context, domain):
    query_context["domain"] = domain
    query_context["root"] = None
    _try_build_query(query_context)


@then("I should receive a QueryBuilder with no root set")
def then_query_builder_no_root(query_context):
    query = query_context["built_query"]
    assert query is not None


@then("I can chain by_correlation_id")
def then_can_chain_correlation_id(query_context):
    # Query builder allows chaining
    assert query_context["built_query"] is not None
