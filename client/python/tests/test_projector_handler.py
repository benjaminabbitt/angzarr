"""Tests for ProjectorHandler."""

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.projector_handler import ProjectorHandler


def _event_book() -> types.EventBook:
    return types.EventBook(
        cover=types.Cover(domain="source"),
        pages=[types.EventPage(num=0), types.EventPage(num=1)],
    )


class TestGetDescriptor:
    def test_returns_name_and_type(self):
        h = ProjectorHandler("test", "domain1", "domain2", "domain3")
        desc = h.descriptor()

        assert desc.name == "test"
        assert desc.component_type == "projector"
        assert len(desc.inputs) == 3
        assert desc.inputs[0].domain == "domain1"
        assert desc.inputs[1].domain == "domain2"
        assert desc.inputs[2].domain == "domain3"

    def test_grpc_descriptor(self):
        h = ProjectorHandler("test", "source")
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
