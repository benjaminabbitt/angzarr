package dev.angzarr.examples.prjoutput;

import java.util.HashMap;
import java.util.Map;

import dev.angzarr.client.StateRouter;
import dev.angzarr.client.annotations.Projects;
import dev.angzarr.examples.CardsDealt;
import dev.angzarr.examples.FundsDeposited;
import dev.angzarr.examples.PlayerRegistered;

/**
 * Output projector examples for documentation.
 *
 * This file contains simplified examples used in the projector documentation,
 * demonstrating both OO-style and StateRouter patterns.
 */

// docs:start:projector_oo
public class OutputProjectorDoc {
    private final Map<String, String> playerNames = new HashMap<>();

    @Projects(PlayerRegistered.class)
    public void handlePlayerRegistered(PlayerRegistered event) {
        playerNames.put(event.getPlayerId(), event.getDisplayName());
        System.out.printf("[Player] %s registered%n", event.getDisplayName());
    }

    @Projects(FundsDeposited.class)
    public void handleFundsDeposited(FundsDeposited event) {
        String name = playerNames.getOrDefault(event.getPlayerId(), event.getPlayerId());
        System.out.printf("[Player] %s deposited $%.2f%n", name, event.getAmount().getAmount() / 100.0);
    }

    @Projects(CardsDealt.class)
    public void handleCardsDealt(CardsDealt event) {
        for (var player : event.getPlayerCardsList()) {
            String name = playerNames.getOrDefault(player.getPlayerId(), player.getPlayerId());
            String cards = formatCards(player.getHoleCardsList());
            System.out.printf("[Hand] %s dealt %s%n", name, cards);
        }
    }

    private String formatCards(java.util.List<?> cards) {
        return "cards"; // Simplified for documentation
    }
}
// docs:end:projector_oo
