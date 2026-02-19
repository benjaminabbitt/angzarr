package dev.angzarr.examples.table.sagaplayer;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import dev.angzarr.*;
import dev.angzarr.client.EventRouter;
import dev.angzarr.examples.*;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Saga: Table -> Player (Functional Pattern)
 *
 * <p>Reacts to HandEnded events from Table domain.
 * Sends ReleaseFunds commands to Player domain.
 */
public final class TablePlayerRouter {

    private TablePlayerRouter() {}

    public static EventRouter createRouter() {
        return new EventRouter("saga-table-player")
            .domain("table")
            .prepare(HandEnded.class, TablePlayerRouter::prepareHandEnded)
            .on(HandEnded.class, TablePlayerRouter::handleHandEnded);
    }

    public static List<Cover> prepareHandEnded(HandEnded event) {
        List<Cover> covers = new ArrayList<>();
        for (String playerHex : event.getStackChangesMap().keySet()) {
            byte[] playerRoot = hexToBytes(playerHex);
            covers.add(Cover.newBuilder()
                .setDomain("player")
                .setRoot(UUID.newBuilder().setValue(ByteString.copyFrom(playerRoot)))
                .build());
        }
        return covers;
    }

    public static CommandBook handleHandEnded(HandEnded event, List<EventBook> destinations) {
        var stackChanges = event.getStackChangesMap();
        if (stackChanges.isEmpty()) {
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

        // Get the first player from the stack changes
        String playerHex = stackChanges.keySet().iterator().next();
        byte[] playerRoot = hexToBytes(playerHex);

        // Get sequence from destination state
        int destSeq = 0;
        if (destMap.containsKey(playerHex)) {
            destSeq = EventRouter.nextSequence(destMap.get(playerHex));
        }

        ReleaseFunds releaseFunds = ReleaseFunds.newBuilder()
            .setTableRoot(event.getHandRoot())
            .build();

        return CommandBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain("player")
                .setRoot(UUID.newBuilder().setValue(ByteString.copyFrom(playerRoot))))
            .addPages(CommandPage.newBuilder()
                .setSequence(destSeq)
                .setCommand(EventRouter.packCommand(releaseFunds)))
            .build();
    }

    private static byte[] hexToBytes(String hex) {
        int len = hex.length();
        byte[] data = new byte[len / 2];
        for (int i = 0; i < len; i += 2) {
            data[i / 2] = (byte) ((Character.digit(hex.charAt(i), 16) << 4)
                + Character.digit(hex.charAt(i + 1), 16));
        }
        return data;
    }

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
