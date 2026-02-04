"""Tests for ProjectorHandler."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from angzarr import types_pb2 as types

from projector_handler import ProjectorHandler


def _event_book() -> types.EventBook:
    return types.EventBook(
        cover=types.Cover(domain="order"),
        pages=[types.EventPage(num=0), types.EventPage(num=1)],
    )


class TestGetDescriptor:
    def test_returns_name_and_type(self):
        h = ProjectorHandler("web", "customer", "order", "product")
        desc = h.descriptor()

        assert desc.name == "web"
        assert desc.component_type == "projector"
        assert len(desc.inputs) == 3
        assert desc.inputs[0].domain == "customer"
        assert desc.inputs[1].domain == "order"
        assert desc.inputs[2].domain == "product"

    def test_grpc_descriptor(self):
        h = ProjectorHandler("test", "order")
        resp = h.GetDescriptor(types.GetDescriptorRequest(), None)

        assert resp.name == "test"
        assert resp.component_type == "projector"
        assert len(resp.inputs) == 1


class TestHandle:
    def test_default_returns_empty_projection(self):
        h = ProjectorHandler("test")
        resp = h.Handle(_event_book(), None)

        assert resp is not None

    def test_custom_returns_projection(self):
        def handle_fn(book):
            return types.Projection(
                projector="test",
                sequence=len(book.pages),
            )

        h = ProjectorHandler("test").with_handle(handle_fn)
        resp = h.Handle(_event_book(), None)

        assert resp.projector == "test"
        assert resp.sequence == 2

    def test_handle_receives_event_book(self):
        received = {}

        def handle_fn(book):
            received["pages"] = len(book.pages)
            return types.Projection()

        h = ProjectorHandler("test").with_handle(handle_fn)
        h.Handle(_event_book(), None)

        assert received["pages"] == 2
