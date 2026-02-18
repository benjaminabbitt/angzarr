package dev.angzarr.examples.hand.sagaplayer;

import com.google.protobuf.Any;
import dev.angzarr.*;
import dev.angzarr.client.router.EventRouter;
import dev.angzarr.examples.*;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Saga: Hand -> Player (Functional Pattern)
 *
 * <p>Reacts to PotAwarded events from Hand domain.
 * Sends DepositFunds commands to Player domain.
 */
public final class HandPlayerRouter {

    private HandPlayerRouter() {}

    public static EventRouter createRouter() {
        return new EventRouter("saga-hand-player", "hand")
            .sends("player", "DepositFunds")
            .prepare(PotAwarded.class, HandPlayerRouter::preparePotAwarded)
            .on(PotAwarded.class, HandPlayerRouter::handlePotAwarded);
    }

    public static List<Cover> preparePotAwarded(PotAwarded event) {
        List<Cover> covers = new ArrayList<>();
        for (PotWinner winner : event.getWinnersList()) {
            covers.add(Cover.newBuilder()
                .setDomain("player")
                .setRoot(UUID.newBuilder().setValue(winner.getPlayerRoot()))
                .build());
        }
        return covers;
    }

    public static CommandBook handlePotAwarded(PotAwarded event, List<EventBook> destinations) {
        if (event.getWinnersCount() == 0) {
            return null;
        }

        // Build destination map for sequence lookup
        Map<String, EventBook> destMap = new HashMap<>();
        for (EventBook dest : destinations) {
            if (dest.hasCover() && dest.getCover().hasRoot()) {
                String key = bytesToHex(dest.getCover().getRoot().getValue().toByteArray());
                destMap.put(key, dest);
            }
        }

        // For a single winner, send a single CommandBook
        PotWinner winner = event.getWinners(0);
        String playerKey = bytesToHex(winner.getPlayerRoot().toByteArray());

        // Get sequence from destination state
        int destSeq = 0;
        if (destMap.containsKey(playerKey)) {
            destSeq = EventRouter.nextSequence(destMap.get(playerKey));
        }

        DepositFunds depositFunds = DepositFunds.newBuilder()
            .setAmount(Currency.newBuilder().setAmount(winner.getAmount()))
            .build();

        return CommandBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain("player")
                .setRoot(UUID.newBuilder().setValue(winner.getPlayerRoot())))
            .addPages(CommandPage.newBuilder()
                .setSequence(destSeq)
                .setCommand(EventRouter.packCommand(depositFunds)))
            .build();
    }

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
