import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
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
    __slots__ = ("points", "new_balance", "reason")
    POINTS_FIELD_NUMBER: _ClassVar[int]
    NEW_BALANCE_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    points: int
    new_balance: int
    reason: str
    def __init__(self, points: _Optional[int] = ..., new_balance: _Optional[int] = ..., reason: _Optional[str] = ...) -> None: ...

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

class CreateTransaction(_message.Message):
    __slots__ = ("customer_id", "items")
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    def __init__(self, customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ...) -> None: ...

class ApplyDiscount(_message.Message):
    __slots__ = ("discount_type", "value", "coupon_code")
    DISCOUNT_TYPE_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    COUPON_CODE_FIELD_NUMBER: _ClassVar[int]
    discount_type: str
    value: int
    coupon_code: str
    def __init__(self, discount_type: _Optional[str] = ..., value: _Optional[int] = ..., coupon_code: _Optional[str] = ...) -> None: ...

class CompleteTransaction(_message.Message):
    __slots__ = ("payment_method",)
    PAYMENT_METHOD_FIELD_NUMBER: _ClassVar[int]
    payment_method: str
    def __init__(self, payment_method: _Optional[str] = ...) -> None: ...

class CancelTransaction(_message.Message):
    __slots__ = ("reason",)
    REASON_FIELD_NUMBER: _ClassVar[int]
    reason: str
    def __init__(self, reason: _Optional[str] = ...) -> None: ...

class TransactionCreated(_message.Message):
    __slots__ = ("customer_id", "items", "subtotal_cents", "created_at")
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    SUBTOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    subtotal_cents: int
    created_at: _timestamp_pb2.Timestamp
    def __init__(self, customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ..., subtotal_cents: _Optional[int] = ..., created_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class DiscountApplied(_message.Message):
    __slots__ = ("discount_type", "value", "discount_cents", "coupon_code")
    DISCOUNT_TYPE_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    COUPON_CODE_FIELD_NUMBER: _ClassVar[int]
    discount_type: str
    value: int
    discount_cents: int
    coupon_code: str
    def __init__(self, discount_type: _Optional[str] = ..., value: _Optional[int] = ..., discount_cents: _Optional[int] = ..., coupon_code: _Optional[str] = ...) -> None: ...

class TransactionCompleted(_message.Message):
    __slots__ = ("final_total_cents", "payment_method", "loyalty_points_earned", "completed_at")
    FINAL_TOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_METHOD_FIELD_NUMBER: _ClassVar[int]
    LOYALTY_POINTS_EARNED_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_AT_FIELD_NUMBER: _ClassVar[int]
    final_total_cents: int
    payment_method: str
    loyalty_points_earned: int
    completed_at: _timestamp_pb2.Timestamp
    def __init__(self, final_total_cents: _Optional[int] = ..., payment_method: _Optional[str] = ..., loyalty_points_earned: _Optional[int] = ..., completed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class TransactionCancelled(_message.Message):
    __slots__ = ("reason", "cancelled_at")
    REASON_FIELD_NUMBER: _ClassVar[int]
    CANCELLED_AT_FIELD_NUMBER: _ClassVar[int]
    reason: str
    cancelled_at: _timestamp_pb2.Timestamp
    def __init__(self, reason: _Optional[str] = ..., cancelled_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class LineItem(_message.Message):
    __slots__ = ("product_id", "name", "quantity", "unit_price_cents")
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    UNIT_PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    name: str
    quantity: int
    unit_price_cents: int
    def __init__(self, product_id: _Optional[str] = ..., name: _Optional[str] = ..., quantity: _Optional[int] = ..., unit_price_cents: _Optional[int] = ...) -> None: ...

class TransactionState(_message.Message):
    __slots__ = ("customer_id", "items", "subtotal_cents", "discount_cents", "discount_type", "status")
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    SUBTOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_TYPE_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    subtotal_cents: int
    discount_cents: int
    discount_type: str
    status: str
    def __init__(self, customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ..., subtotal_cents: _Optional[int] = ..., discount_cents: _Optional[int] = ..., discount_type: _Optional[str] = ..., status: _Optional[str] = ...) -> None: ...

class Receipt(_message.Message):
    __slots__ = ("transaction_id", "customer_id", "items", "subtotal_cents", "discount_cents", "final_total_cents", "payment_method", "loyalty_points_earned", "completed_at", "formatted_text")
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    SUBTOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    FINAL_TOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_METHOD_FIELD_NUMBER: _ClassVar[int]
    LOYALTY_POINTS_EARNED_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_AT_FIELD_NUMBER: _ClassVar[int]
    FORMATTED_TEXT_FIELD_NUMBER: _ClassVar[int]
    transaction_id: str
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    subtotal_cents: int
    discount_cents: int
    final_total_cents: int
    payment_method: str
    loyalty_points_earned: int
    completed_at: _timestamp_pb2.Timestamp
    formatted_text: str
    def __init__(self, transaction_id: _Optional[str] = ..., customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ..., subtotal_cents: _Optional[int] = ..., discount_cents: _Optional[int] = ..., final_total_cents: _Optional[int] = ..., payment_method: _Optional[str] = ..., loyalty_points_earned: _Optional[int] = ..., completed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., formatted_text: _Optional[str] = ...) -> None: ...
