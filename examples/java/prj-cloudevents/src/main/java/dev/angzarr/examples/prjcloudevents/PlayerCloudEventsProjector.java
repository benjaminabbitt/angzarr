package dev.angzarr.examples.prjcloudevents;

import com.google.protobuf.Any;
import dev.angzarr.client.CloudEventsProjector;
import dev.angzarr.client.CloudEventsRouter;
import dev.angzarr.proto.angzarr.CloudEvent;
import dev.angzarr.proto.examples.FundsDeposited;
import dev.angzarr.proto.examples.PlayerRegistered;
import dev.angzarr.proto.examples.PublicFundsDeposited;
import dev.angzarr.proto.examples.PublicPlayerRegistered;

/**
 * CloudEvents projector - publishes player events as CloudEvents.
 *
 * This projector transforms internal domain events into CloudEvents 1.0 format
 * for external consumption via HTTP webhooks or Kafka.
 */

// docs:start:cloudevents_oo
public class PlayerCloudEventsProjector extends CloudEventsProjector {

    public PlayerCloudEventsProjector() {
        super("prj-player-cloudevents", "player");
    }

    @Publishes("PlayerRegistered")
    public CloudEvent onPlayerRegistered(PlayerRegistered event) {
        // Filter sensitive fields, return public version
        var publicEvent = PublicPlayerRegistered.newBuilder()
            .setDisplayName(event.getDisplayName())
            .setPlayerType(event.getPlayerType())
            .build();
        return CloudEvent.newBuilder()
            .setType("com.poker.player.registered")
            .setData(Any.pack(publicEvent))
            .build();
    }

    @Publishes("FundsDeposited")
    public CloudEvent onFundsDeposited(FundsDeposited event) {
        var publicEvent = PublicFundsDeposited.newBuilder()
            .setAmount(event.getAmount())
            .build();
        return CloudEvent.newBuilder()
            .setType("com.poker.player.deposited")
            .setData(Any.pack(publicEvent))
            .putExtensions("priority", "normal")
            .build();
    }
}
// docs:end:cloudevents_oo
