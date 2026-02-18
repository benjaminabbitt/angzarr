package dev.angzarr.examples.prjcloudevents;

import com.google.protobuf.Any;
import dev.angzarr.client.CloudEventsRouter;
import dev.angzarr.proto.angzarr.CloudEvent;
import dev.angzarr.proto.examples.FundsDeposited;
import dev.angzarr.proto.examples.PlayerRegistered;
import dev.angzarr.proto.examples.PublicFundsDeposited;
import dev.angzarr.proto.examples.PublicPlayerRegistered;

/**
 * CloudEvents router pattern - functional style.
 */

// docs:start:cloudevents_router
class PlayerCloudEventsHandlers {

    static CloudEvent handlePlayerRegistered(PlayerRegistered event) {
        var publicEvent = PublicPlayerRegistered.newBuilder()
            .setDisplayName(event.getDisplayName())
            .setPlayerType(event.getPlayerType())
            .build();
        return CloudEvent.newBuilder()
            .setType("com.poker.player.registered")
            .setData(Any.pack(publicEvent))
            .build();
    }

    static CloudEvent handleFundsDeposited(FundsDeposited event) {
        var publicEvent = PublicFundsDeposited.newBuilder()
            .setAmount(event.getAmount())
            .build();
        return CloudEvent.newBuilder()
            .setType("com.poker.player.deposited")
            .setData(Any.pack(publicEvent))
            .putExtensions("priority", "normal")
            .build();
    }

    static CloudEventsRouter buildRouter() {
        return new CloudEventsRouter("prj-player-cloudevents", "player")
            .on(PlayerRegistered.class, PlayerCloudEventsHandlers::handlePlayerRegistered)
            .on(FundsDeposited.class, PlayerCloudEventsHandlers::handleFundsDeposited);
    }
}
// docs:end:cloudevents_router
