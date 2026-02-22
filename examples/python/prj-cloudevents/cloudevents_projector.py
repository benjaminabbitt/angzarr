"""CloudEvents projector - publishes player events as CloudEvents.

This projector transforms internal domain events into CloudEvents 1.0 format
for external consumption via HTTP webhooks or Kafka.
"""

from google.protobuf.any_pb2 import Any

from angzarr_client import CloudEventsProjector, CloudEventsRouter, CloudEvent
from angzarr_client.proto.examples import player_pb2 as player


# docs:start:cloudevents_oo
class PlayerCloudEventsProjector(CloudEventsProjector):
    """Publishes public player events to external consumers."""

    def __init__(self):
        super().__init__("prj-player-cloudevents", "player")

    def on_player_registered(self, event: player.PlayerRegistered) -> CloudEvent | None:
        # Filter sensitive fields, return public version
        public = player.PublicPlayerRegistered(
            display_name=event.display_name,
            player_type=event.player_type,
            # Omit: email (PII)
        )
        data = Any()
        data.Pack(public)
        return CloudEvent(type="com.poker.player.registered", data=data)

    def on_funds_deposited(self, event: player.FundsDeposited) -> CloudEvent | None:
        public = player.PublicFundsDeposited(
            amount=event.amount,
        )
        data = Any()
        data.Pack(public)
        return CloudEvent(
            type="com.poker.player.deposited",
            data=data,
            extensions={"priority": "normal"},
        )


# docs:end:cloudevents_oo


# docs:start:cloudevents_router
def handle_player_registered(event: player.PlayerRegistered) -> CloudEvent | None:
    public = player.PublicPlayerRegistered(
        display_name=event.display_name,
        player_type=event.player_type,
    )
    data = Any()
    data.Pack(public)
    return CloudEvent(type="com.poker.player.registered", data=data)


def handle_funds_deposited(event: player.FundsDeposited) -> CloudEvent | None:
    public = player.PublicFundsDeposited(amount=event.amount)
    data = Any()
    data.Pack(public)
    return CloudEvent(
        type="com.poker.player.deposited",
        data=data,
        extensions={"priority": "normal"},
    )


router = (
    CloudEventsRouter("prj-player-cloudevents", "player")
    .on("PlayerRegistered", handle_player_registered)
    .on("FundsDeposited", handle_funds_deposited)
)
# docs:end:cloudevents_router
