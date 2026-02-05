import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class CreateCustomer(_message.Message):
    __slots__ = ("name", "email")
    NAME_FIELD_NUMBER: _ClassVar[int]
    EMAIL_FIELD_NUMBER: _ClassVar[int]
    name: str
    email: str
    def __init__(self, name: _Optional[str] = ..., email: _Optional[str] = ...) -> None: ...

class AddLoyaltyPoints(_message.Message):
    __slots__ = ("points", "reason")
    POINTS_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    points: int
    reason: str
    def __init__(self, points: _Optional[int] = ..., reason: _Optional[str] = ...) -> None: ...

class RedeemLoyaltyPoints(_message.Message):
    __slots__ = ("points", "redemption_type")
    POINTS_FIELD_NUMBER: _ClassVar[int]
    REDEMPTION_TYPE_FIELD_NUMBER: _ClassVar[int]
    points: int
    redemption_type: str
    def __init__(self, points: _Optional[int] = ..., redemption_type: _Optional[str] = ...) -> None: ...

class CustomerCreated(_message.Message):
    __slots__ = ("name", "email", "created_at")
    NAME_FIELD_NUMBER: _ClassVar[int]
    EMAIL_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    name: str
    email: str
    created_at: _timestamp_pb2.Timestamp
    def __init__(self, name: _Optional[str] = ..., email: _Optional[str] = ..., created_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class LoyaltyPointsAdded(_message.Message):
    __slots__ = ("points", "new_balance", "reason", "new_lifetime_points")
    POINTS_FIELD_NUMBER: _ClassVar[int]
    NEW_BALANCE_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    NEW_LIFETIME_POINTS_FIELD_NUMBER: _ClassVar[int]
    points: int
    new_balance: int
    reason: str
    new_lifetime_points: int
    def __init__(self, points: _Optional[int] = ..., new_balance: _Optional[int] = ..., reason: _Optional[str] = ..., new_lifetime_points: _Optional[int] = ...) -> None: ...

class LoyaltyPointsRedeemed(_message.Message):
    __slots__ = ("points", "new_balance", "redemption_type")
    POINTS_FIELD_NUMBER: _ClassVar[int]
    NEW_BALANCE_FIELD_NUMBER: _ClassVar[int]
    REDEMPTION_TYPE_FIELD_NUMBER: _ClassVar[int]
    points: int
    new_balance: int
    redemption_type: str
    def __init__(self, points: _Optional[int] = ..., new_balance: _Optional[int] = ..., redemption_type: _Optional[str] = ...) -> None: ...

class CustomerState(_message.Message):
    __slots__ = ("name", "email", "loyalty_points", "lifetime_points")
    NAME_FIELD_NUMBER: _ClassVar[int]
    EMAIL_FIELD_NUMBER: _ClassVar[int]
    LOYALTY_POINTS_FIELD_NUMBER: _ClassVar[int]
    LIFETIME_POINTS_FIELD_NUMBER: _ClassVar[int]
    name: str
    email: str
    loyalty_points: int
    lifetime_points: int
    def __init__(self, name: _Optional[str] = ..., email: _Optional[str] = ..., loyalty_points: _Optional[int] = ..., lifetime_points: _Optional[int] = ...) -> None: ...
