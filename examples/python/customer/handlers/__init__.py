"""Command handlers for Customer bounded context.

Contains command handlers for customer lifecycle and loyalty points management.
"""


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


from handlers.create_customer import handle_create_customer
from handlers.add_loyalty_points import handle_add_loyalty_points
from handlers.redeem_loyalty_points import handle_redeem_loyalty_points

__all__ = [
    "CommandRejectedError",
    "handle_create_customer",
    "handle_add_loyalty_points",
    "handle_redeem_loyalty_points",
]
