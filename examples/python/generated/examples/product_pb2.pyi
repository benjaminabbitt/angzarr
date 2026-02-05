import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class CreateProduct(_message.Message):
    __slots__ = ("sku", "name", "description", "price_cents")
    SKU_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    sku: str
    name: str
    description: str
    price_cents: int
    def __init__(self, sku: _Optional[str] = ..., name: _Optional[str] = ..., description: _Optional[str] = ..., price_cents: _Optional[int] = ...) -> None: ...

class UpdateProduct(_message.Message):
    __slots__ = ("name", "description")
    NAME_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    name: str
    description: str
    def __init__(self, name: _Optional[str] = ..., description: _Optional[str] = ...) -> None: ...

class SetPrice(_message.Message):
    __slots__ = ("price_cents",)
    PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    price_cents: int
    def __init__(self, price_cents: _Optional[int] = ...) -> None: ...

class Discontinue(_message.Message):
    __slots__ = ("reason",)
    REASON_FIELD_NUMBER: _ClassVar[int]
    reason: str
    def __init__(self, reason: _Optional[str] = ...) -> None: ...

class ProductCreated(_message.Message):
    __slots__ = ("sku", "name", "description", "price_cents", "created_at")
    SKU_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    sku: str
    name: str
    description: str
    price_cents: int
    created_at: _timestamp_pb2.Timestamp
    def __init__(self, sku: _Optional[str] = ..., name: _Optional[str] = ..., description: _Optional[str] = ..., price_cents: _Optional[int] = ..., created_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class ProductUpdated(_message.Message):
    __slots__ = ("name", "description", "updated_at")
    NAME_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    UPDATED_AT_FIELD_NUMBER: _ClassVar[int]
    name: str
    description: str
    updated_at: _timestamp_pb2.Timestamp
    def __init__(self, name: _Optional[str] = ..., description: _Optional[str] = ..., updated_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class PriceSet(_message.Message):
    __slots__ = ("price_cents", "previous_price_cents", "set_at")
    PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    PREVIOUS_PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    SET_AT_FIELD_NUMBER: _ClassVar[int]
    price_cents: int
    previous_price_cents: int
    set_at: _timestamp_pb2.Timestamp
    def __init__(self, price_cents: _Optional[int] = ..., previous_price_cents: _Optional[int] = ..., set_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class ProductDiscontinued(_message.Message):
    __slots__ = ("reason", "discontinued_at")
    REASON_FIELD_NUMBER: _ClassVar[int]
    DISCONTINUED_AT_FIELD_NUMBER: _ClassVar[int]
    reason: str
    discontinued_at: _timestamp_pb2.Timestamp
    def __init__(self, reason: _Optional[str] = ..., discontinued_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class ProductState(_message.Message):
    __slots__ = ("sku", "name", "description", "price_cents", "status")
    SKU_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    sku: str
    name: str
    description: str
    price_cents: int
    status: str
    def __init__(self, sku: _Optional[str] = ..., name: _Optional[str] = ..., description: _Optional[str] = ..., price_cents: _Optional[int] = ..., status: _Optional[str] = ...) -> None: ...
