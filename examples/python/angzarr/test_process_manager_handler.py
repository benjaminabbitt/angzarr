"""Tests for ProcessManagerHandler."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from angzarr import process_manager_pb2 as pm
from angzarr import types_pb2 as types

from process_manager_handler import ProcessManagerHandler


def _trigger() -> types.EventBook:
    return types.EventBook(
        cover=types.Cover(domain="order", root=types.UUID(value=b"root-1")),
    )


class TestGetDescriptor:
    def test_returns_name_and_type(self):
        h = (
            ProcessManagerHandler("order-fulfillment")
            .listen_to("order", "OrderCompleted")
            .listen_to("inventory", "StockReserved")
        )
        desc = h.descriptor()

        assert desc.name == "order-fulfillment"
        assert desc.component_type == "process_manager"
        assert len(desc.inputs) == 2
        assert desc.inputs[0].domain == "order"
        assert desc.inputs[1].domain == "inventory"

    def test_grpc_descriptor(self):
        h = ProcessManagerHandler("test").listen_to("order", "OrderCompleted")
        resp = h.GetDescriptor(types.GetDescriptorRequest(), None)

        assert resp.name == "test"
        assert resp.component_type == "process_manager"
        assert len(resp.inputs) == 1


class TestPrepare:
    def test_default_returns_empty(self):
        h = ProcessManagerHandler("test")
        req = pm.ProcessManagerPrepareRequest(trigger=_trigger())

        resp = h.Prepare(req, None)

        assert len(resp.destinations) == 0

    def test_custom_returns_destinations(self):
        def prepare_fn(trigger, process_state):
            return [types.Cover(domain="fulfillment", root=trigger.cover.root)]

        h = ProcessManagerHandler("test").with_prepare(prepare_fn)
        req = pm.ProcessManagerPrepareRequest(trigger=_trigger())

        resp = h.Prepare(req, None)

        assert len(resp.destinations) == 1
        assert resp.destinations[0].domain == "fulfillment"


class TestHandle:
    def test_default_returns_empty(self):
        h = ProcessManagerHandler("test")
        req = pm.ProcessManagerHandleRequest(trigger=_trigger())

        resp = h.Handle(req, None)

        assert len(resp.commands) == 0

    def test_custom_returns_commands_and_events(self):
        def handle_fn(trigger, process_state, destinations):
            commands = [
                types.CommandBook(cover=types.Cover(domain="fulfillment")),
            ]
            events = types.EventBook(
                pages=[types.EventPage(num=0)],
            )
            return commands, events

        h = ProcessManagerHandler("test").with_handle(handle_fn)
        req = pm.ProcessManagerHandleRequest(trigger=_trigger())

        resp = h.Handle(req, None)

        assert len(resp.commands) == 1
        assert resp.commands[0].cover.domain == "fulfillment"
        assert len(resp.process_events.pages) == 1

    def test_handle_receives_destinations(self):
        received = {}

        def handle_fn(trigger, process_state, destinations):
            received["destinations"] = destinations
            return [], None

        h = ProcessManagerHandler("test").with_handle(handle_fn)
        dest = types.EventBook(pages=[types.EventPage(), types.EventPage()])
        req = pm.ProcessManagerHandleRequest(
            trigger=_trigger(),
            destinations=[dest],
        )

        h.Handle(req, None)

        assert len(received["destinations"]) == 1
        assert len(received["destinations"][0].pages) == 2
