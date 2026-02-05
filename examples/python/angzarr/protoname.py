"""Proto type name helpers using DESCRIPTOR reflection.

Provides utilities to extract type names from proto messages without
hardcoding strings.

Example::

    from proto import order_pb2
    from protoname import name, type_url

    # Extract short type name
    name(order_pb2.CreateOrder)  # "CreateOrder"

    # Build full type URL
    type_url(order_pb2.CreateOrder)  # "type.examples/examples.CreateOrder"
"""

from typing import Type, Union

from google.protobuf.message import Message

TYPE_URL_PREFIX = "type.examples/examples."


def name(msg_or_cls: Union[Message, Type[Message]]) -> str:
    """Extract the short type name from a proto message or class.

    Args:
        msg_or_cls: Either a proto message instance or message class.

    Returns:
        The short type name (e.g., "CreateOrder").
    """
    if isinstance(msg_or_cls, type):
        return msg_or_cls.DESCRIPTOR.name
    return msg_or_cls.DESCRIPTOR.name


def type_url(msg_or_cls: Union[Message, Type[Message]]) -> str:
    """Build the full type URL for a proto message or class.

    Args:
        msg_or_cls: Either a proto message instance or message class.

    Returns:
        The full type URL (e.g., "type.examples/examples.CreateOrder").
    """
    return TYPE_URL_PREFIX + name(msg_or_cls)
