"""Handler errors and error message constants."""


class errmsg:
    """Error message constants for cart domain."""

    CART_EXISTS = "Cart already exists"
    CART_NOT_FOUND = "Cart does not exist"
    CART_CHECKED_OUT = "Cart is already checked out"
    CART_EMPTY = "Cart is empty"
    ITEM_NOT_IN_CART = "Item not in cart"
    QUANTITY_POSITIVE = "Quantity must be positive"
    COUPON_ALREADY_APPLIED = "Coupon already applied"
    UNKNOWN_COMMAND = "Unknown command type"
    NO_COMMAND_PAGES = "CommandBook has no pages"
    CUSTOMER_ID_REQUIRED = "Customer ID is required"
    PRODUCT_ID_REQUIRED = "Product ID is required"
    COUPON_CODE_REQUIRED = "Coupon code is required"
    PERCENTAGE_RANGE = "Percentage must be 0-100"
    FIXED_DISCOUNT_NEGATIVE = "Fixed discount cannot be negative"
    INVALID_COUPON_TYPE = "Invalid coupon type"


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""
