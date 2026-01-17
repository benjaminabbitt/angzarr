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
    __slots__ = ("quantity", "order_id", "new_available", "reserved_at")
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    NEW_AVAILABLE_FIELD_NUMBER: _ClassVar[int]
    RESERVED_AT_FIELD_NUMBER: _ClassVar[int]
    quantity: int
    order_id: str
    new_available: int
    reserved_at: _timestamp_pb2.Timestamp
    def __init__(self, quantity: _Optional[int] = ..., order_id: _Optional[str] = ..., new_available: _Optional[int] = ..., reserved_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class ReservationReleased(_message.Message):
    __slots__ = ("order_id", "quantity", "new_available", "released_at")
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    NEW_AVAILABLE_FIELD_NUMBER: _ClassVar[int]
    RELEASED_AT_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    quantity: int
    new_available: int
    released_at: _timestamp_pb2.Timestamp
    def __init__(self, order_id: _Optional[str] = ..., quantity: _Optional[int] = ..., new_available: _Optional[int] = ..., released_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class ReservationCommitted(_message.Message):
    __slots__ = ("order_id", "quantity", "new_on_hand", "committed_at")
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    NEW_ON_HAND_FIELD_NUMBER: _ClassVar[int]
    COMMITTED_AT_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    quantity: int
    new_on_hand: int
    committed_at: _timestamp_pb2.Timestamp
    def __init__(self, order_id: _Optional[str] = ..., quantity: _Optional[int] = ..., new_on_hand: _Optional[int] = ..., committed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

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

class CreateOrder(_message.Message):
    __slots__ = ("customer_id", "items")
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    def __init__(self, customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ...) -> None: ...

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
    __slots__ = ("final_total_cents", "payment_method", "payment_reference", "loyalty_points_earned", "completed_at")
    FINAL_TOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_METHOD_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_REFERENCE_FIELD_NUMBER: _ClassVar[int]
    LOYALTY_POINTS_EARNED_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_AT_FIELD_NUMBER: _ClassVar[int]
    final_total_cents: int
    payment_method: str
    payment_reference: str
    loyalty_points_earned: int
    completed_at: _timestamp_pb2.Timestamp
    def __init__(self, final_total_cents: _Optional[int] = ..., payment_method: _Optional[str] = ..., payment_reference: _Optional[str] = ..., loyalty_points_earned: _Optional[int] = ..., completed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class OrderCancelled(_message.Message):
    __slots__ = ("reason", "cancelled_at", "loyalty_points_used")
    REASON_FIELD_NUMBER: _ClassVar[int]
    CANCELLED_AT_FIELD_NUMBER: _ClassVar[int]
    LOYALTY_POINTS_USED_FIELD_NUMBER: _ClassVar[int]
    reason: str
    cancelled_at: _timestamp_pb2.Timestamp
    loyalty_points_used: int
    def __init__(self, reason: _Optional[str] = ..., cancelled_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., loyalty_points_used: _Optional[int] = ...) -> None: ...

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

class OrderState(_message.Message):
    __slots__ = ("customer_id", "items", "subtotal_cents", "discount_cents", "loyalty_points_used", "payment_method", "payment_reference", "status")
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    SUBTOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    LOYALTY_POINTS_USED_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_METHOD_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_REFERENCE_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    subtotal_cents: int
    discount_cents: int
    loyalty_points_used: int
    payment_method: str
    payment_reference: str
    status: str
    def __init__(self, customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ..., subtotal_cents: _Optional[int] = ..., discount_cents: _Optional[int] = ..., loyalty_points_used: _Optional[int] = ..., payment_method: _Optional[str] = ..., payment_reference: _Optional[str] = ..., status: _Optional[str] = ...) -> None: ...

class CreateCart(_message.Message):
    __slots__ = ("customer_id",)
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    def __init__(self, customer_id: _Optional[str] = ...) -> None: ...

class AddItem(_message.Message):
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

class UpdateQuantity(_message.Message):
    __slots__ = ("product_id", "new_quantity")
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    NEW_QUANTITY_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    new_quantity: int
    def __init__(self, product_id: _Optional[str] = ..., new_quantity: _Optional[int] = ...) -> None: ...

class RemoveItem(_message.Message):
    __slots__ = ("product_id",)
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    def __init__(self, product_id: _Optional[str] = ...) -> None: ...

class ApplyCoupon(_message.Message):
    __slots__ = ("code", "coupon_type", "value")
    CODE_FIELD_NUMBER: _ClassVar[int]
    COUPON_TYPE_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    code: str
    coupon_type: str
    value: int
    def __init__(self, code: _Optional[str] = ..., coupon_type: _Optional[str] = ..., value: _Optional[int] = ...) -> None: ...

class ClearCart(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class Checkout(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class CartCreated(_message.Message):
    __slots__ = ("customer_id", "created_at")
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    created_at: _timestamp_pb2.Timestamp
    def __init__(self, customer_id: _Optional[str] = ..., created_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class ItemAdded(_message.Message):
    __slots__ = ("product_id", "name", "quantity", "unit_price_cents", "new_subtotal", "added_at")
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    UNIT_PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    NEW_SUBTOTAL_FIELD_NUMBER: _ClassVar[int]
    ADDED_AT_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    name: str
    quantity: int
    unit_price_cents: int
    new_subtotal: int
    added_at: _timestamp_pb2.Timestamp
    def __init__(self, product_id: _Optional[str] = ..., name: _Optional[str] = ..., quantity: _Optional[int] = ..., unit_price_cents: _Optional[int] = ..., new_subtotal: _Optional[int] = ..., added_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class QuantityUpdated(_message.Message):
    __slots__ = ("product_id", "old_quantity", "new_quantity", "new_subtotal", "updated_at")
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    OLD_QUANTITY_FIELD_NUMBER: _ClassVar[int]
    NEW_QUANTITY_FIELD_NUMBER: _ClassVar[int]
    NEW_SUBTOTAL_FIELD_NUMBER: _ClassVar[int]
    UPDATED_AT_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    old_quantity: int
    new_quantity: int
    new_subtotal: int
    updated_at: _timestamp_pb2.Timestamp
    def __init__(self, product_id: _Optional[str] = ..., old_quantity: _Optional[int] = ..., new_quantity: _Optional[int] = ..., new_subtotal: _Optional[int] = ..., updated_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class ItemRemoved(_message.Message):
    __slots__ = ("product_id", "quantity", "new_subtotal", "removed_at")
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    NEW_SUBTOTAL_FIELD_NUMBER: _ClassVar[int]
    REMOVED_AT_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    quantity: int
    new_subtotal: int
    removed_at: _timestamp_pb2.Timestamp
    def __init__(self, product_id: _Optional[str] = ..., quantity: _Optional[int] = ..., new_subtotal: _Optional[int] = ..., removed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class CouponApplied(_message.Message):
    __slots__ = ("coupon_code", "coupon_type", "value", "discount_cents", "applied_at")
    COUPON_CODE_FIELD_NUMBER: _ClassVar[int]
    COUPON_TYPE_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    APPLIED_AT_FIELD_NUMBER: _ClassVar[int]
    coupon_code: str
    coupon_type: str
    value: int
    discount_cents: int
    applied_at: _timestamp_pb2.Timestamp
    def __init__(self, coupon_code: _Optional[str] = ..., coupon_type: _Optional[str] = ..., value: _Optional[int] = ..., discount_cents: _Optional[int] = ..., applied_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class CartCleared(_message.Message):
    __slots__ = ("new_subtotal", "cleared_at")
    NEW_SUBTOTAL_FIELD_NUMBER: _ClassVar[int]
    CLEARED_AT_FIELD_NUMBER: _ClassVar[int]
    new_subtotal: int
    cleared_at: _timestamp_pb2.Timestamp
    def __init__(self, new_subtotal: _Optional[int] = ..., cleared_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class CartCheckedOut(_message.Message):
    __slots__ = ("final_subtotal", "discount_cents", "checked_out_at")
    FINAL_SUBTOTAL_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    CHECKED_OUT_AT_FIELD_NUMBER: _ClassVar[int]
    final_subtotal: int
    discount_cents: int
    checked_out_at: _timestamp_pb2.Timestamp
    def __init__(self, final_subtotal: _Optional[int] = ..., discount_cents: _Optional[int] = ..., checked_out_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class CartItem(_message.Message):
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

class CartState(_message.Message):
    __slots__ = ("customer_id", "items", "subtotal_cents", "coupon_code", "discount_cents", "status")
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    SUBTOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    COUPON_CODE_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    items: _containers.RepeatedCompositeFieldContainer[CartItem]
    subtotal_cents: int
    coupon_code: str
    discount_cents: int
    status: str
    def __init__(self, customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[CartItem, _Mapping]]] = ..., subtotal_cents: _Optional[int] = ..., coupon_code: _Optional[str] = ..., discount_cents: _Optional[int] = ..., status: _Optional[str] = ...) -> None: ...

class CreateShipment(_message.Message):
    __slots__ = ("order_id",)
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    def __init__(self, order_id: _Optional[str] = ...) -> None: ...

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
    __slots__ = ("order_id", "status", "created_at")
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    status: str
    created_at: _timestamp_pb2.Timestamp
    def __init__(self, order_id: _Optional[str] = ..., status: _Optional[str] = ..., created_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

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
    __slots__ = ("carrier", "tracking_number", "shipped_at")
    CARRIER_FIELD_NUMBER: _ClassVar[int]
    TRACKING_NUMBER_FIELD_NUMBER: _ClassVar[int]
    SHIPPED_AT_FIELD_NUMBER: _ClassVar[int]
    carrier: str
    tracking_number: str
    shipped_at: _timestamp_pb2.Timestamp
    def __init__(self, carrier: _Optional[str] = ..., tracking_number: _Optional[str] = ..., shipped_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class Delivered(_message.Message):
    __slots__ = ("signature", "delivered_at")
    SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    DELIVERED_AT_FIELD_NUMBER: _ClassVar[int]
    signature: str
    delivered_at: _timestamp_pb2.Timestamp
    def __init__(self, signature: _Optional[str] = ..., delivered_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class FulfillmentState(_message.Message):
    __slots__ = ("order_id", "status", "tracking_number", "carrier", "picker_id", "packer_id", "signature")
    ORDER_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    TRACKING_NUMBER_FIELD_NUMBER: _ClassVar[int]
    CARRIER_FIELD_NUMBER: _ClassVar[int]
    PICKER_ID_FIELD_NUMBER: _ClassVar[int]
    PACKER_ID_FIELD_NUMBER: _ClassVar[int]
    SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    order_id: str
    status: str
    tracking_number: str
    carrier: str
    picker_id: str
    packer_id: str
    signature: str
    def __init__(self, order_id: _Optional[str] = ..., status: _Optional[str] = ..., tracking_number: _Optional[str] = ..., carrier: _Optional[str] = ..., picker_id: _Optional[str] = ..., packer_id: _Optional[str] = ..., signature: _Optional[str] = ...) -> None: ...

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
    items: _containers.RepeatedCompositeFieldContainer[LineItem]
    subtotal_cents: int
    discount_cents: int
    final_total_cents: int
    payment_method: str
    loyalty_points_earned: int
    completed_at: _timestamp_pb2.Timestamp
    formatted_text: str
    def __init__(self, order_id: _Optional[str] = ..., customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[LineItem, _Mapping]]] = ..., subtotal_cents: _Optional[int] = ..., discount_cents: _Optional[int] = ..., final_total_cents: _Optional[int] = ..., payment_method: _Optional[str] = ..., loyalty_points_earned: _Optional[int] = ..., completed_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., formatted_text: _Optional[str] = ...) -> None: ...
