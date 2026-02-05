import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class InitializeStock(_message.Message):
    __slots__ = ("product_id", "quantity", "low_stock_threshold")
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    LOW_STOCK_THRESHOLD_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    quantity: int
    low_stock_threshold: int
    def __init__(self, product_id: _Optional[str] = ..., quantity: _Optional[int] = ..., low_stock_threshold: _Optional[int] = ...) -> None: ...

class ReceiveStock(_message.Message):
    __slots__ = ("quantity", "reference")
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    REFERENCE_FIELD_NUMBER: _ClassVar[int]
    quantity: int
    reference: str
    def __init__(self, quantity: _Optional[int] = ..., reference: _Optional[str] = ...) -> None: ...

class ReserveStock(_message.Message):
    __slots__ = ("quantity", "order_id")
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    quantity: int
    order_id: str
    def __init__(self, quantity: _Optional[int] = ..., order_id: _Optional[str] = ...) -> None: ...

class ReleaseReservation(_message.Message):
    __slots__ = ("order_id",)
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    def __init__(self, order_id: _Optional[str] = ...) -> None: ...

class CommitReservation(_message.Message):
    __slots__ = ("order_id",)
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    def __init__(self, order_id: _Optional[str] = ...) -> None: ...

class StockInitialized(_message.Message):
    __slots__ = ("product_id", "quantity", "low_stock_threshold", "initialized_at")
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    LOW_STOCK_THRESHOLD_FIELD_NUMBER: _ClassVar[int]
    INITIALIZED_AT_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    quantity: int
    low_stock_threshold: int
    initialized_at: _timestamp_pb2.Timestamp
    def __init__(self, product_id: _Optional[str] = ..., quantity: _Optional[int] = ..., low_stock_threshold: _Optional[int] = ..., initialized_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class StockReceived(_message.Message):
    __slots__ = ("quantity", "new_on_hand", "reference", "received_at")
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    NEW_ON_HAND_FIELD_NUMBER: _ClassVar[int]
    REFERENCE_FIELD_NUMBER: _ClassVar[int]
    RECEIVED_AT_FIELD_NUMBER: _ClassVar[int]
    quantity: int
    new_on_hand: int
    reference: str
    received_at: _timestamp_pb2.Timestamp
    def __init__(self, quantity: _Optional[int] = ..., new_on_hand: _Optional[int] = ..., reference: _Optional[str] = ..., received_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class StockReserved(_message.Message):
    __slots__ = ("quantity", "order_id", "new_available", "reserved_at", "new_reserved", "new_on_hand")
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    NEW_AVAILABLE_FIELD_NUMBER: _ClassVar[int]
    RESERVED_AT_FIELD_NUMBER: _ClassVar[int]
    NEW_RESERVED_FIELD_NUMBER: _ClassVar[int]
    NEW_ON_HAND_FIELD_NUMBER: _ClassVar[int]
    quantity: int
    order_id: str
    new_available: int
    reserved_at: _timestamp_pb2.Timestamp
    new_reserved: int
    new_on_hand: int
    def __init__(self, quantity: _Optional[int] = ..., order_id: _Optional[str] = ..., new_available: _Optional[int] = ..., reserved_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., new_reserved: _Optional[int] = ..., new_on_hand: _Optional[int] = ...) -> None: ...

class ReservationReleased(_message.Message):
    __slots__ = ("order_id", "quantity", "new_available", "released_at", "new_reserved", "new_on_hand")
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    NEW_AVAILABLE_FIELD_NUMBER: _ClassVar[int]
    RELEASED_AT_FIELD_NUMBER: _ClassVar[int]
    NEW_RESERVED_FIELD_NUMBER: _ClassVar[int]
    NEW_ON_HAND_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    quantity: int
    new_available: int
    released_at: _timestamp_pb2.Timestamp
    new_reserved: int
    new_on_hand: int
    def __init__(self, order_id: _Optional[str] = ..., quantity: _Optional[int] = ..., new_available: _Optional[int] = ..., released_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., new_reserved: _Optional[int] = ..., new_on_hand: _Optional[int] = ...) -> None: ...

class ReservationCommitted(_message.Message):
    __slots__ = ("order_id", "quantity", "new_on_hand", "committed_at", "new_reserved")
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    NEW_ON_HAND_FIELD_NUMBER: _ClassVar[int]
    COMMITTED_AT_FIELD_NUMBER: _ClassVar[int]
    NEW_RESERVED_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    quantity: int
    new_on_hand: int
    committed_at: _timestamp_pb2.Timestamp
    new_reserved: int
    def __init__(self, order_id: _Optional[str] = ..., quantity: _Optional[int] = ..., new_on_hand: _Optional[int] = ..., committed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., new_reserved: _Optional[int] = ...) -> None: ...

class LowStockAlert(_message.Message):
    __slots__ = ("product_id", "available", "threshold", "alerted_at")
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    AVAILABLE_FIELD_NUMBER: _ClassVar[int]
    THRESHOLD_FIELD_NUMBER: _ClassVar[int]
    ALERTED_AT_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    available: int
    threshold: int
    alerted_at: _timestamp_pb2.Timestamp
    def __init__(self, product_id: _Optional[str] = ..., available: _Optional[int] = ..., threshold: _Optional[int] = ..., alerted_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class InventoryState(_message.Message):
    __slots__ = ("product_id", "on_hand", "reserved", "low_stock_threshold", "reservations")
    class ReservationsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: int
        def __init__(self, key: _Optional[str] = ..., value: _Optional[int] = ...) -> None: ...
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    ON_HAND_FIELD_NUMBER: _ClassVar[int]
    RESERVED_FIELD_NUMBER: _ClassVar[int]
    LOW_STOCK_THRESHOLD_FIELD_NUMBER: _ClassVar[int]
    RESERVATIONS_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    on_hand: int
    reserved: int
    low_stock_threshold: int
    reservations: _containers.ScalarMap[str, int]
    def __init__(self, product_id: _Optional[str] = ..., on_hand: _Optional[int] = ..., reserved: _Optional[int] = ..., low_stock_threshold: _Optional[int] = ..., reservations: _Optional[_Mapping[str, int]] = ...) -> None: ...
