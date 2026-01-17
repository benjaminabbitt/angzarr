"""Product command handlers."""

from .create_product import handle_create_product
from .discontinue import handle_discontinue
from .exceptions import CommandRejectedError
from .set_price import handle_set_price
from .update_product import handle_update_product

__all__ = [
    "CommandRejectedError",
    "handle_create_product",
    "handle_discontinue",
    "handle_set_price",
    "handle_update_product",
]
