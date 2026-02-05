from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Iterable as _Iterable, Mapping as _Mapping, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class AddItem(_message.Message):
    __slots__ = ["name", "product_id", "quantity", "unit_price_cents"]
    NAME_FIELD_NUMBER: _ClassVar[int]
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    UNIT_PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    name: str
    product_id: str
    quantity: int
    unit_price_cents: int
    def __init__(self, product_id: _Optional[str] = ..., name: _Optional[str] = ..., quantity: _Optional[int] = ..., unit_price_cents: _Optional[int] = ...) -> None: ...

class ApplyCoupon(_message.Message):
    __slots__ = ["code", "coupon_type", "value"]
    CODE_FIELD_NUMBER: _ClassVar[int]
    COUPON_TYPE_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    code: str
    coupon_type: str
    value: int
    def __init__(self, code: _Optional[str] = ..., coupon_type: _Optional[str] = ..., value: _Optional[int] = ...) -> None: ...

class CartCheckedOut(_message.Message):
    __slots__ = ["checked_out_at", "customer_id", "discount_cents", "final_subtotal", "items"]
    CHECKED_OUT_AT_FIELD_NUMBER: _ClassVar[int]
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    FINAL_SUBTOTAL_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    checked_out_at: _timestamp_pb2.Timestamp
    customer_id: str
    discount_cents: int
    final_subtotal: int
    items: _containers.RepeatedCompositeFieldContainer[CartItem]
    def __init__(self, final_subtotal: _Optional[int] = ..., discount_cents: _Optional[int] = ..., checked_out_at: _Optional[_Union[_timestamp_pb2.Timestamp, _Mapping]] = ..., customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[CartItem, _Mapping]]] = ...) -> None: ...

class CartCleared(_message.Message):
    __slots__ = ["cleared_at", "items", "new_subtotal"]
    CLEARED_AT_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    NEW_SUBTOTAL_FIELD_NUMBER: _ClassVar[int]
    cleared_at: _timestamp_pb2.Timestamp
    items: _containers.RepeatedCompositeFieldContainer[CartItem]
    new_subtotal: int
    def __init__(self, new_subtotal: _Optional[int] = ..., cleared_at: _Optional[_Union[_timestamp_pb2.Timestamp, _Mapping]] = ..., items: _Optional[_Iterable[_Union[CartItem, _Mapping]]] = ...) -> None: ...

class CartCreated(_message.Message):
    __slots__ = ["created_at", "customer_id"]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    created_at: _timestamp_pb2.Timestamp
    customer_id: str
    def __init__(self, customer_id: _Optional[str] = ..., created_at: _Optional[_Union[_timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class CartItem(_message.Message):
    __slots__ = ["name", "product_id", "quantity", "unit_price_cents"]
    NAME_FIELD_NUMBER: _ClassVar[int]
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    UNIT_PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    name: str
    product_id: str
    quantity: int
    unit_price_cents: int
    def __init__(self, product_id: _Optional[str] = ..., name: _Optional[str] = ..., quantity: _Optional[int] = ..., unit_price_cents: _Optional[int] = ...) -> None: ...

class CartState(_message.Message):
    __slots__ = ["coupon_code", "customer_id", "discount_cents", "items", "status", "subtotal_cents"]
    COUPON_CODE_FIELD_NUMBER: _ClassVar[int]
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    SUBTOTAL_CENTS_FIELD_NUMBER: _ClassVar[int]
    coupon_code: str
    customer_id: str
    discount_cents: int
    items: _containers.RepeatedCompositeFieldContainer[CartItem]
    status: str
    subtotal_cents: int
    def __init__(self, customer_id: _Optional[str] = ..., items: _Optional[_Iterable[_Union[CartItem, _Mapping]]] = ..., subtotal_cents: _Optional[int] = ..., coupon_code: _Optional[str] = ..., discount_cents: _Optional[int] = ..., status: _Optional[str] = ...) -> None: ...

class Checkout(_message.Message):
    __slots__ = []
    def __init__(self) -> None: ...

class ClearCart(_message.Message):
    __slots__ = []
    def __init__(self) -> None: ...

class CouponApplied(_message.Message):
    __slots__ = ["applied_at", "coupon_code", "coupon_type", "discount_cents", "value"]
    APPLIED_AT_FIELD_NUMBER: _ClassVar[int]
    COUPON_CODE_FIELD_NUMBER: _ClassVar[int]
    COUPON_TYPE_FIELD_NUMBER: _ClassVar[int]
    DISCOUNT_CENTS_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    applied_at: _timestamp_pb2.Timestamp
    coupon_code: str
    coupon_type: str
    discount_cents: int
    value: int
    def __init__(self, coupon_code: _Optional[str] = ..., coupon_type: _Optional[str] = ..., value: _Optional[int] = ..., discount_cents: _Optional[int] = ..., applied_at: _Optional[_Union[_timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class CreateCart(_message.Message):
    __slots__ = ["customer_id"]
    CUSTOMER_ID_FIELD_NUMBER: _ClassVar[int]
    customer_id: str
    def __init__(self, customer_id: _Optional[str] = ...) -> None: ...

class ItemAdded(_message.Message):
    __slots__ = ["added_at", "name", "new_subtotal", "product_id", "quantity", "unit_price_cents"]
    ADDED_AT_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    NEW_SUBTOTAL_FIELD_NUMBER: _ClassVar[int]
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    UNIT_PRICE_CENTS_FIELD_NUMBER: _ClassVar[int]
    added_at: _timestamp_pb2.Timestamp
    name: str
    new_subtotal: int
    product_id: str
    quantity: int
    unit_price_cents: int
    def __init__(self, product_id: _Optional[str] = ..., name: _Optional[str] = ..., quantity: _Optional[int] = ..., unit_price_cents: _Optional[int] = ..., new_subtotal: _Optional[int] = ..., added_at: _Optional[_Union[_timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class ItemRemoved(_message.Message):
    __slots__ = ["new_subtotal", "product_id", "quantity", "removed_at"]
    NEW_SUBTOTAL_FIELD_NUMBER: _ClassVar[int]
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    QUANTITY_FIELD_NUMBER: _ClassVar[int]
    REMOVED_AT_FIELD_NUMBER: _ClassVar[int]
    new_subtotal: int
    product_id: str
    quantity: int
    removed_at: _timestamp_pb2.Timestamp
    def __init__(self, product_id: _Optional[str] = ..., quantity: _Optional[int] = ..., new_subtotal: _Optional[int] = ..., removed_at: _Optional[_Union[_timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class QuantityUpdated(_message.Message):
    __slots__ = ["new_quantity", "new_subtotal", "old_quantity", "product_id", "updated_at"]
    NEW_QUANTITY_FIELD_NUMBER: _ClassVar[int]
    NEW_SUBTOTAL_FIELD_NUMBER: _ClassVar[int]
    OLD_QUANTITY_FIELD_NUMBER: _ClassVar[int]
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    UPDATED_AT_FIELD_NUMBER: _ClassVar[int]
    new_quantity: int
    new_subtotal: int
    old_quantity: int
    product_id: str
    updated_at: _timestamp_pb2.Timestamp
    def __init__(self, product_id: _Optional[str] = ..., old_quantity: _Optional[int] = ..., new_quantity: _Optional[int] = ..., new_subtotal: _Optional[int] = ..., updated_at: _Optional[_Union[_timestamp_pb2.Timestamp, _Mapping]] = ...) -> None: ...

class RemoveItem(_message.Message):
    __slots__ = ["product_id"]
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    product_id: str
    def __init__(self, product_id: _Optional[str] = ...) -> None: ...

class UpdateQuantity(_message.Message):
    __slots__ = ["new_quantity", "product_id"]
    NEW_QUANTITY_FIELD_NUMBER: _ClassVar[int]
    PRODUCT_ID_FIELD_NUMBER: _ClassVar[int]
    new_quantity: int
    product_id: str
    def __init__(self, product_id: _Optional[str] = ..., new_quantity: _Optional[int] = ...) -> None: ...
