package dev.angzarr.examples.table.sagaplayer;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import dev.angzarr.*;
import dev.angzarr.client.Saga;
import dev.angzarr.client.annotations.Prepares;
import dev.angzarr.client.annotations.ReactsTo;
import dev.angzarr.examples.*;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Saga: Table -> Player (OO Pattern)
 *
 * <p>Reacts to HandEnded events from Table domain.
 * Sends ReleaseFunds commands to Player domain.
 */
public class TablePlayerSaga extends Saga {

    public TablePlayerSaga() {
        super("saga-table-player", "table", "player");
    }

    @Prepares(HandEnded.class)
    public List<Cover> prepareHandEnded(HandEnded event) {
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

    @ReactsTo(HandEnded.class)
    public List<CommandBook> handleHandEnded(HandEnded event, List<EventBook> destinations) {
        // Build destination map for sequence lookup
        Map<String, EventBook> destMap = new HashMap<>();
        for (EventBook dest : destinations) {
            if (dest.hasCover() && dest.getCover().hasRoot()) {
                String key = bytesToHex(dest.getCover().getRoot().getValue().toByteArray());
                destMap.put(key, dest);
            }
        }

        List<CommandBook> commands = new ArrayList<>();

        for (String playerHex : event.getStackChangesMap().keySet()) {
            byte[] playerRoot = hexToBytes(playerHex);

            // Get sequence from destination state
            int destSeq = 0;
            if (destMap.containsKey(playerHex)) {
                destSeq = Saga.nextSequence(destMap.get(playerHex));
            }

            ReleaseFunds releaseFunds = ReleaseFunds.newBuilder()
                .setTableRoot(event.getHandRoot())
                .build();

            commands.add(CommandBook.newBuilder()
                .setCover(Cover.newBuilder()
                    .setDomain("player")
                    .setRoot(UUID.newBuilder().setValue(ByteString.copyFrom(playerRoot))))
                .addPages(CommandPage.newBuilder()
                    .setSequence(destSeq)
                    .setCommand(Any.pack(releaseFunds, "type.googleapis.com/")))
                .build());
        }

        return commands;
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
