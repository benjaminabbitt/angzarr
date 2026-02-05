import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from examples import order_pb2 as _order_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Receipt(_message.Message):
    __slots__ = ("order_id", "customer_id", "items", "subtotal_cents", "discount_cents", "final_total_cents", "payment_method", "loyalty_points_earned", "completed_at", "formatted_text")
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    SUBTOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    FINAL_TOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_METHOD_FIELD_NUMBER: _ClassVar[int]
    LOYALTY_POINTS_EARNED_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_AT_FIELD_NUMBER: _ClassVar[int]
    FORMATTED_TEXT_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[_order_pb2.LineItem]
    subtotal_cents: int
    discount_cents: int
    final_total_cents: int
    payment_method: str
    loyalty_points_earned: int
    completed_at: _timestamp_pb2.Timestamp
    formatted_text: str
    def __init__(self, order_id: _Optional[str] = ..., customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[_order_pb2.LineItem, _Mapping]]] = ..., subtotal_cents: _Optional[int] = ..., discount_cents: _Optional[int] = ..., final_total_cents: _Optional[int] = ..., payment_method: _Optional[str] = ..., loyalty_points_earned: _Optional[int] = ..., completed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., formatted_text: _Optional[str] = ...) -> None: ...
