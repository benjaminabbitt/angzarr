package dev.angzarr.examples.prjoutput;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import dev.angzarr.*;
import dev.angzarr.examples.*;

import java.io.FileWriter;
import java.io.IOException;
import java.io.PrintWriter;
import java.time.Instant;
import java.time.ZoneOffset;
import java.time.format.DateTimeFormatter;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;

/**
 * Projector: Output
 *
 * <p>Subscribes to events from player, table, and hand domains
 * and writes formatted game logs.
 */
public class OutputProjector {

    private static final Map<String, Class<?>> EVENT_TYPES = new HashMap<>();
    static {
        // Player events
        EVENT_TYPES.put("PlayerRegistered", PlayerRegistered.class);
        EVENT_TYPES.put("FundsDeposited", FundsDeposited.class);
        EVENT_TYPES.put("FundsWithdrawn", FundsWithdrawn.class);
        EVENT_TYPES.put("FundsReserved", FundsReserved.class);
        EVENT_TYPES.put("FundsReleased", FundsReleased.class);
        // Table events
        EVENT_TYPES.put("TableCreated", TableCreated.class);
        EVENT_TYPES.put("PlayerJoined", PlayerJoined.class);
        EVENT_TYPES.put("PlayerLeft", PlayerLeft.class);
        EVENT_TYPES.put("HandStarted", HandStarted.class);
        EVENT_TYPES.put("HandEnded", HandEnded.class);
        // Hand events
        EVENT_TYPES.put("CardsDealt", CardsDealt.class);
        EVENT_TYPES.put("BlindPosted", BlindPosted.class);
        EVENT_TYPES.put("ActionTaken", ActionTaken.class);
        EVENT_TYPES.put("CommunityCardsDealt", CommunityCardsDealt.class);
        EVENT_TYPES.put("PotAwarded", PotAwarded.class);
        EVENT_TYPES.put("HandComplete", HandComplete.class);
    }

    private final TextRenderer renderer;
    private final Consumer<String> outputFn;
    private final boolean showTimestamps;
    private final DateTimeFormatter timeFormatter;

    public OutputProjector(Consumer<String> outputFn, boolean showTimestamps) {
        this.renderer = new TextRenderer();
        this.outputFn = outputFn;
        this.showTimestamps = showTimestamps;
        this.timeFormatter = DateTimeFormatter.ofPattern("HH:mm:ss").withZone(ZoneOffset.UTC);
    }

    /**
     * Get list of domains this projector subscribes to.
     */
    public List<String> getInputDomains() {
        return List.of("player", "table", "hand");
    }

    /**
     * Set display name for a player root.
     */
    public void setPlayerName(byte[] playerRoot, String name) {
        renderer.setPlayerName(playerRoot, name);
    }

    /**
     * Handle an event book and return a projection.
     */
    public Projection handle(EventBook eventBook) {
        handleEventBook(eventBook);

        // Return a projection with the sequence number
        int seq = 0;
        if (eventBook.getPagesCount() > 0) {
            EventPage lastPage = eventBook.getPages(eventBook.getPagesCount() - 1);
            seq = lastPage.getNum();
        }

        return Projection.newBuilder()
            .setCover(eventBook.getCover())
            .setProjector("output")
            .setSequence(seq)
            .build();
    }

    /**
     * Handle all events in an event book.
     */
    public void handleEventBook(EventBook eventBook) {
        for (EventPage page : eventBook.getPagesList()) {
            handleEvent(page);
        }
    }

    /**
     * Handle a single event page from any domain.
     */
    public void handleEvent(EventPage eventPage) {
        Any eventAny = eventPage.getEvent();
        String typeUrl = eventAny.getTypeUrl();

        // Extract event type from type_url
        String eventType = extractEventType(typeUrl);

        if (!EVENT_TYPES.containsKey(eventType)) {
            outputFn.accept("[Unknown event type: " + typeUrl + "]");
            return;
        }

        try {
            // Unpack the event
            @SuppressWarnings("unchecked")
            Class<? extends com.google.protobuf.Message> eventClass =
                (Class<? extends com.google.protobuf.Message>) EVENT_TYPES.get(eventType);
            Object event = eventAny.unpack(eventClass);

            // Render and output
            String text = renderer.render(eventType, event);
            if (text != null && !text.isEmpty()) {
                if (showTimestamps && eventPage.hasCreatedAt()) {
                    Instant ts = Instant.ofEpochSecond(
                        eventPage.getCreatedAt().getSeconds(),
                        eventPage.getCreatedAt().getNanos()
                    );
                    text = "[" + timeFormatter.format(ts) + "] " + text;
                }
                outputFn.accept(text);
            }
        } catch (InvalidProtocolBufferException e) {
            outputFn.accept("[Failed to unpack: " + typeUrl + "]");
        }
    }

    private String extractEventType(String typeUrl) {
        int dotIndex = typeUrl.lastIndexOf('.');
        if (dotIndex >= 0) {
            return typeUrl.substring(dotIndex + 1);
        }
        return typeUrl;
    }

    /**
     * Create a file-based projector.
     */
    public static OutputProjector forFile(String path, boolean showTimestamps) throws IOException {
        PrintWriter writer = new PrintWriter(new FileWriter(path, true));
        return new OutputProjector(line -> {
            writer.println(line);
            writer.flush();
        }, showTimestamps);
    }

    /**
     * Create a console-based projector.
     */
    public static OutputProjector forConsole(boolean showTimestamps) {
        return new OutputProjector(System.out::println, showTimestamps);
    }
}
