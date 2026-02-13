"""Proto type name helpers using DESCRIPTOR reflection.

Provides utilities to extract type names from proto messages without
hardcoding strings.

Example::

    from angzarr_client.proto.examples import player_pb2
    from angzarr_client import protoname

    # Extract short type name
    protoname.name(player_pb2.RegisterPlayer)  # "RegisterPlayer"

    # Build full type URL
    protoname.type_url(player_pb2.RegisterPlayer)  # "type.examples/examples.RegisterPlayer"
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
