package dev.angzarr.examples.hand.sagaplayer;

import com.google.protobuf.Any;
import dev.angzarr.*;
import dev.angzarr.client.Helpers;
import dev.angzarr.client.Saga;
import dev.angzarr.client.annotations.Prepares;
import dev.angzarr.client.annotations.ReactsTo;
import dev.angzarr.examples.*;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Saga: Hand -> Player (OO Pattern)
 *
 * <p>Reacts to PotAwarded events from Hand domain.
 * Sends DepositFunds commands to Player domain.
 */
public class HandPlayerSaga extends Saga {

    public HandPlayerSaga() {
        super("saga-hand-player", "hand", "player");
    }

    @Prepares(PotAwarded.class)
    public List<Cover> preparePotAwarded(PotAwarded event) {
        List<Cover> covers = new ArrayList<>();
        for (PotWinner winner : event.getWinnersList()) {
            covers.add(Cover.newBuilder()
                .setDomain("player")
                .setRoot(UUID.newBuilder().setValue(winner.getPlayerRoot()))
                .build());
        }
        return covers;
    }

    @ReactsTo(PotAwarded.class)
    public List<CommandBook> handlePotAwarded(PotAwarded event, List<EventBook> destinations) {
        // Build destination map for sequence lookup
        Map<String, EventBook> destMap = new HashMap<>();
        for (EventBook dest : destinations) {
            if (dest.hasCover() && dest.getCover().hasRoot()) {
                String key = bytesToHex(dest.getCover().getRoot().getValue().toByteArray());
                destMap.put(key, dest);
            }
        }

        List<CommandBook> commands = new ArrayList<>();

        for (PotWinner winner : event.getWinnersList()) {
            String playerKey = bytesToHex(winner.getPlayerRoot().toByteArray());

            // Get sequence from destination state
            int destSeq = 0;
            if (destMap.containsKey(playerKey)) {
                destSeq = Helpers.nextSequence(destMap.get(playerKey));
            }

            DepositFunds depositFunds = DepositFunds.newBuilder()
                .setAmount(Currency.newBuilder().setAmount(winner.getAmount()))
                .build();

            commands.add(CommandBook.newBuilder()
                .setCover(Cover.newBuilder()
                    .setDomain("player")
                    .setRoot(UUID.newBuilder().setValue(winner.getPlayerRoot())))
                .addPages(CommandPage.newBuilder()
                    .setSequence(destSeq)
                    .setCommand(Any.pack(depositFunds, "type.googleapis.com/")))
                .build());
        }

        return commands;
    }

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
