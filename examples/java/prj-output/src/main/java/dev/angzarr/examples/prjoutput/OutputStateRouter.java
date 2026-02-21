package dev.angzarr.examples.prjoutput;

import java.util.HashMap;
import java.util.Map;

import dev.angzarr.client.StateRouter;
import dev.angzarr.examples.CardsDealt;
import dev.angzarr.examples.FundsDeposited;
import dev.angzarr.examples.PlayerRegistered;

/**
 * StateRouter pattern for documentation.
 */

// docs:start:state_router
class OutputStateRouterExample {
    private static final Map<String, String> playerNames = new HashMap<>();

    static void handlePlayerRegistered(PlayerRegistered event) {
        playerNames.put(event.getPlayerId(), event.getDisplayName());
        System.out.printf("[Player] %s registered%n", event.getDisplayName());
    }

    static void handleFundsDeposited(FundsDeposited event) {
        String name = playerNames.getOrDefault(event.getPlayerId(), event.getPlayerId());
        System.out.printf("[Player] %s deposited $%.2f%n", name, event.getAmount().getAmount() / 100.0);
    }

    static void handleCardsDealt(CardsDealt event) {
        for (var player : event.getPlayerCardsList()) {
            String name = playerNames.getOrDefault(player.getPlayerId(), player.getPlayerId());
            System.out.printf("[Hand] %s dealt cards%n", name);
        }
    }

    static StateRouter buildRouter() {
        return new StateRouter("prj-output")
            .subscribes("player", new String[]{"PlayerRegistered", "FundsDeposited"})
            .subscribes("hand", new String[]{"CardsDealt", "ActionTaken", "PotAwarded"})
            .on(PlayerRegistered.class, OutputStateRouterExample::handlePlayerRegistered)
            .on(FundsDeposited.class, OutputStateRouterExample::handleFundsDeposited)
            .on(CardsDealt.class, OutputStateRouterExample::handleCardsDealt);
    }
}
// docs:end:state_router
