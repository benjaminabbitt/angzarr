import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from examples import order_pb2 as _order_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class CreateShipment(_message.Message):
    __slots__ = ("order_id", "items")
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    items: _containers.RepeatedCompositeFieldContainer[_order_pb2.LineItem]
    def __init__(self, order_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[_order_pb2.LineItem, _Mapping]]] = ...) -> None: ...

class MarkPicked(_message.Message):
    __slots__ = ("picker_id",)
    PICKER_ID_FIELD_NUMBER: _ClassVar[int]
    picker_id: str
    def __init__(self, picker_id: _Optional[str] = ...) -> None: ...

class MarkPacked(_message.Message):
    __slots__ = ("packer_id",)
    PACKER_ID_FIELD_NUMBER: _ClassVar[int]
    packer_id: str
    def __init__(self, packer_id: _Optional[str] = ...) -> None: ...

class Ship(_message.Message):
    __slots__ = ("carrier", "tracking_number")
    CARRIER_FIELD_NUMBER: _ClassVar[int]
    TRACKING_NUMBER_FIELD_NUMBER: _ClassVar[int]
    carrier: str
    tracking_number: str
    def __init__(self, carrier: _Optional[str] = ..., tracking_number: _Optional[str] = ...) -> None: ...

class RecordDelivery(_message.Message):
    __slots__ = ("signature",)
    SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    signature: str
    def __init__(self, signature: _Optional[str] = ...) -> None: ...

class ShipmentCreated(_message.Message):
    __slots__ = ("order_id", "status", "created_at", "items")
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    status: str
    created_at: _timestamp_pb2.Timestamp
    items: _containers.RepeatedCompositeFieldContainer[_order_pb2.LineItem]
    def __init__(self, order_id: _Optional[str] = ..., status: _Optional[str] = ..., created_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., items: _Optional[_Iterable[_Union[_order_pb2.LineItem, _Mapping]]] = ...) -> None: ...

class ItemsPicked(_message.Message):
    __slots__ = ("picker_id", "picked_at")
    PICKER_ID_FIELD_NUMBER: _ClassVar[int]
    PICKED_AT_FIELD_NUMBER: _ClassVar[int]
    picker_id: str
    picked_at: _timestamp_pb2.Timestamp
    def __init__(self, picker_id: _Optional[str] = ..., picked_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class ItemsPacked(_message.Message):
    __slots__ = ("packer_id", "packed_at")
    PACKER_ID_FIELD_NUMBER: _ClassVar[int]
    PACKED_AT_FIELD_NUMBER: _ClassVar[int]
    packer_id: str
    packed_at: _timestamp_pb2.Timestamp
    def __init__(self, packer_id: _Optional[str] = ..., packed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class Shipped(_message.Message):
    __slots__ = ("carrier", "tracking_number", "shipped_at", "items", "order_id")
    CARRIER_FIELD_NUMBER: _ClassVar[int]
    TRACKING_NUMBER_FIELD_NUMBER: _ClassVar[int]
    SHIPPED_AT_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    carrier: str
    tracking_number: str
    shipped_at: _timestamp_pb2.Timestamp
    items: _containers.RepeatedCompositeFieldContainer[_order_pb2.LineItem]
    order_id: str
    def __init__(self, carrier: _Optional[str] = ..., tracking_number: _Optional[str] = ..., shipped_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., items: _Optional[_Iterable[_Union[_order_pb2.LineItem, _Mapping]]] = ..., order_id: _Optional[str] = ...) -> None: ...

class Delivered(_message.Message):
    __slots__ = ("signature", "delivered_at")
    SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    DELIVERED_AT_FIELD_NUMBER: _ClassVar[int]
    signature: str
    delivered_at: _timestamp_pb2.Timestamp
    def __init__(self, signature: _Optional[str] = ..., delivered_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class FulfillmentState(_message.Message):
    __slots__ = ("order_id", "status", "tracking_number", "carrier", "picker_id", "packer_id", "signature", "items")
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    TRACKING_NUMBER_FIELD_NUMBER: _ClassVar[int]
    CARRIER_FIELD_NUMBER: _ClassVar[int]
    PICKER_ID_FIELD_NUMBER: _ClassVar[int]
    PACKER_ID_FIELD_NUMBER: _ClassVar[int]
    SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    status: str
    tracking_number: str
    carrier: str
    picker_id: str
    packer_id: str
    signature: str
    items: _containers.RepeatedCompositeFieldContainer[_order_pb2.LineItem]
    def __init__(self, order_id: _Optional[str] = ..., status: _Optional[str] = ..., tracking_number: _Optional[str] = ..., carrier: _Optional[str] = ..., picker_id: _Optional[str] = ..., packer_id: _Optional[str] = ..., signature: _Optional[str] = ..., items: _Optional[_Iterable[_Union[_order_pb2.LineItem, _Mapping]]] = ...) -> None: ...
