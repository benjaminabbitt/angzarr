import datetime

from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class CreateOrder(_message.Message):
    __slots__ = ("customer_id", "items", "customer_root", "cart_root")
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    CUSTOMER_ROOT_FIELD_NUMBER: _ClassVar[int]
    CART_ROOT_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    customer_root: bytes
    cart_root: bytes
    def __init__(self, customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ..., customer_root: _Optional[bytes] = ..., cart_root: _Optional[bytes] = ...) -> None: ...

class ApplyLoyaltyDiscount(_message.Message):
    __slots__ = ("points", "discount_cents")
    POINTS_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    points: int
    discount_cents: int
    def __init__(self, points: _Optional[int] = ..., discount_cents: _Optional[int] = ...) -> None: ...

class SubmitPayment(_message.Message):
    __slots__ = ("payment_method", "amount_cents")
    PAYMENT_METHOD_FIELD_NUMBER: _ClassVar[int]
    AMOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    payment_method: str
    amount_cents: int
    def __init__(self, payment_method: _Optional[str] = ..., amount_cents: _Optional[int] = ...) -> None: ...

class ConfirmPayment(_message.Message):
    __slots__ = ("payment_reference",)
    PAYMENT_REFERENCE_FIELD_NUMBER: _ClassVar[int]
    payment_reference: str
    def __init__(self, payment_reference: _Optional[str] = ...) -> None: ...

class CancelOrder(_message.Message):
    __slots__ = ("reason",)
    REASON_FIELD_NUMBER: _ClassVar[int]
    reason: str
    def __init__(self, reason: _Optional[str] = ...) -> None: ...

class OrderCreated(_message.Message):
    __slots__ = ("customer_id", "items", "subtotal_cents", "created_at", "customer_root", "cart_root")
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    SUBTOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    CUSTOMER_ROOT_FIELD_NUMBER: _ClassVar[int]
    CART_ROOT_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    subtotal_cents: int
    created_at: _timestamp_pb2.Timestamp
    customer_root: bytes
    cart_root: bytes
    def __init__(self, customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ..., subtotal_cents: _Optional[int] = ..., created_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., customer_root: _Optional[bytes] = ..., cart_root: _Optional[bytes] = ...) -> None: ...

class LoyaltyDiscountApplied(_message.Message):
    __slots__ = ("points_used", "discount_cents", "applied_at")
    POINTS_USED_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    APPLIED_AT_FIELD_NUMBER: _ClassVar[int]
    points_used: int
    discount_cents: int
    applied_at: _timestamp_pb2.Timestamp
    def __init__(self, points_used: _Optional[int] = ..., discount_cents: _Optional[int] = ..., applied_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class PaymentSubmitted(_message.Message):
    __slots__ = ("payment_method", "amount_cents", "submitted_at")
    PAYMENT_METHOD_FIELD_NUMBER: _ClassVar[int]
    AMOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    SUBMITTED_AT_FIELD_NUMBER: _ClassVar[int]
    payment_method: str
    amount_cents: int
    submitted_at: _timestamp_pb2.Timestamp
    def __init__(self, payment_method: _Optional[str] = ..., amount_cents: _Optional[int] = ..., submitted_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class OrderCompleted(_message.Message):
    __slots__ = ("final_total_cents", "payment_method", "payment_reference", "loyalty_points_earned", "completed_at", "customer_root", "cart_root", "items")
    FINAL_TOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_METHOD_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_REFERENCE_FIELD_NUMBER: _ClassVar[int]
    LOYALTY_POINTS_EARNED_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_AT_FIELD_NUMBER: _ClassVar[int]
    CUSTOMER_ROOT_FIELD_NUMBER: _ClassVar[int]
    CART_ROOT_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    final_total_cents: int
    payment_method: str
    payment_reference: str
    loyalty_points_earned: int
    completed_at: _timestamp_pb2.Timestamp
    customer_root: bytes
    cart_root: bytes
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    def __init__(self, final_total_cents: _Optional[int] = ..., payment_method: _Optional[str] = ..., payment_reference: _Optional[str] = ..., loyalty_points_earned: _Optional[int] = ..., completed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., customer_root: _Optional[bytes] = ..., cart_root: _Optional[bytes] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ...) -> None: ...

class OrderCancelled(_message.Message):
    __slots__ = ("reason", "cancelled_at", "loyalty_points_used", "customer_root", "items", "cart_root")
    REASON_FIELD_NUMBER: _ClassVar[int]
    CANCELLED_AT_FIELD_NUMBER: _ClassVar[int]
    LOYALTY_POINTS_USED_FIELD_NUMBER: _ClassVar[int]
    CUSTOMER_ROOT_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    CART_ROOT_FIELD_NUMBER: _ClassVar[int]
    reason: str
    cancelled_at: _timestamp_pb2.Timestamp
    loyalty_points_used: int
    customer_root: bytes
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    cart_root: bytes
    def __init__(self, reason: _Optional[str] = ..., cancelled_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., loyalty_points_used: _Optional[int] = ..., customer_root: _Optional[bytes] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ..., cart_root: _Optional[bytes] = ...) -> None: ...

class LineItem(_message.Message):
    __slots__ = ("product_id", "name", "quantity", "unit_price_cents", "product_root")
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    UNIT_PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    PRODUCT_ROOT_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    name: str
    quantity: int
    unit_price_cents: int
    product_root: bytes
    def __init__(self, product_id: _Optional[str] = ..., name: _Optional[str] = ..., quantity: _Optional[int] = ..., unit_price_cents: _Optional[int] = ..., product_root: _Optional[bytes] = ...) -> None: ...

class OrderState(_message.Message):
    __slots__ = ("customer_id", "items", "subtotal_cents", "discount_cents", "loyalty_points_used", "payment_method", "payment_reference", "status", "customer_root", "cart_root")
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    SUBTOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    LOYALTY_POINTS_USED_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_METHOD_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_REFERENCE_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    CUSTOMER_ROOT_FIELD_NUMBER: _ClassVar[int]
    CART_ROOT_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    subtotal_cents: int
    discount_cents: int
    loyalty_points_used: int
    payment_method: str
    payment_reference: str
    status: str
    customer_root: bytes
    cart_root: bytes
    def __init__(self, customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ..., subtotal_cents: _Optional[int] = ..., discount_cents: _Optional[int] = ..., loyalty_points_used: _Optional[int] = ..., payment_method: _Optional[str] = ..., payment_reference: _Optional[str] = ..., status: _Optional[str] = ..., customer_root: _Optional[bytes] = ..., cart_root: _Optional[bytes] = ...) -> None: ...
