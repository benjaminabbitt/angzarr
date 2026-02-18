package dev.angzarr.examples.prjoutputoo;

import dev.angzarr.client.Projection;
import dev.angzarr.client.Projector;
import dev.angzarr.client.annotations.Projects;
import dev.angzarr.examples.*;

import java.io.*;
import java.time.Instant;
import java.time.format.DateTimeFormatter;
import java.util.List;
import java.util.stream.Collectors;

/**
 * Projector: Output (OO Pattern)
 *
 * <p>Subscribes to player, table, and hand domain events.
 * Writes formatted game logs to a file.
 *
 * <p>This is the OO-style implementation using Projector base class with
 * {@code @Projects} annotated methods.
 */
// docs:start:projector_oo
public class OutputProjector extends Projector {

    private static final String LOG_FILE = System.getenv().getOrDefault(
        "HAND_LOG_FILE", "hand_log_oo.txt"
    );

    private static PrintWriter logWriter;

    public OutputProjector() {
        super("output", List.of("player", "table", "hand"));
    }

    private static synchronized PrintWriter getLogWriter() {
        if (logWriter == null) {
            try {
                logWriter = new PrintWriter(
                    new BufferedWriter(new FileWriter(LOG_FILE, true))
                );
            } catch (IOException e) {
                System.err.println("Failed to open log file: " + e.getMessage());
            }
        }
        return logWriter;
    }

    private void writeLog(String msg) {
        PrintWriter writer = getLogWriter();
        if (writer != null) {
            String timestamp = DateTimeFormatter.ISO_INSTANT.format(Instant.now());
            writer.println("[" + timestamp + "] " + msg);
            writer.flush();
        }
    }

    private String truncateId(com.google.protobuf.ByteString playerRoot) {
        byte[] bytes = playerRoot.toByteArray();
        if (bytes.length >= 4) {
            return String.format("%02x%02x%02x%02x",
                bytes[0] & 0xFF, bytes[1] & 0xFF, bytes[2] & 0xFF, bytes[3] & 0xFF);
        }
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b & 0xFF));
        }
        return sb.toString();
    }

    @Projects(PlayerRegistered.class)
    public Projection projectRegistered(PlayerRegistered event) {
        writeLog(String.format("PLAYER registered: %s (%s)",
            event.getDisplayName(), event.getEmail()));
        return Projection.upsert("log", "registered");
    }

    @Projects(FundsDeposited.class)
    public Projection projectDeposited(FundsDeposited event) {
        long amount = event.hasAmount() ? event.getAmount().getAmount() : 0;
        long newBalance = event.hasNewBalance() ? event.getNewBalance().getAmount() : 0;
        writeLog(String.format("PLAYER deposited %d, balance: %d", amount, newBalance));
        return Projection.upsert("log", "deposited");
    }

    @Projects(TableCreated.class)
    public Projection projectTableCreated(TableCreated event) {
        writeLog(String.format("TABLE created: %s (%s)",
            event.getTableName(), event.getGameVariant()));
        return Projection.upsert("log", "table_created");
    }

    @Projects(PlayerJoined.class)
    public Projection projectPlayerJoined(PlayerJoined event) {
        String playerId = truncateId(event.getPlayerRoot());
        writeLog(String.format("TABLE player %s joined with %d chips",
            playerId, event.getStack()));
        return Projection.upsert("log", "player_joined");
    }

    @Projects(HandStarted.class)
    public Projection projectHandStarted(HandStarted event) {
        writeLog(String.format("TABLE hand #%d started, %d players, dealer at position %d",
            event.getHandNumber(), event.getActivePlayersCount(), event.getDealerPosition()));
        return Projection.upsert("log", "hand_started");
    }

    @Projects(CardsDealt.class)
    public Projection projectCardsDealt(CardsDealt event) {
        writeLog(String.format("HAND cards dealt to %d players",
            event.getPlayerCardsCount()));
        return Projection.upsert("log", "cards_dealt");
    }

    @Projects(BlindPosted.class)
    public Projection projectBlindPosted(BlindPosted event) {
        String playerId = truncateId(event.getPlayerRoot());
        writeLog(String.format("HAND player %s posted %s blind: %d",
            playerId, event.getBlindType(), event.getAmount()));
        return Projection.upsert("log", "blind_posted");
    }

    @Projects(ActionTaken.class)
    public Projection projectActionTaken(ActionTaken event) {
        String playerId = truncateId(event.getPlayerRoot());
        writeLog(String.format("HAND player %s: %s %d",
            playerId, event.getAction(), event.getAmount()));
        return Projection.upsert("log", "action_taken");
    }

    @Projects(PotAwarded.class)
    public Projection projectPotAwarded(PotAwarded event) {
        String winners = event.getWinnersList().stream()
            .map(w -> truncateId(w.getPlayerRoot()) + " wins " + w.getAmount())
            .collect(Collectors.joining(", "));
        writeLog(String.format("HAND pot awarded: %s", winners));
        return Projection.upsert("log", "pot_awarded");
    }

    @Projects(HandComplete.class)
    public Projection projectHandComplete(HandComplete event) {
        writeLog(String.format("HAND #%d complete", event.getHandNumber()));
        return Projection.upsert("log", "hand_complete");
    }
}
// docs:end:projector_oo
