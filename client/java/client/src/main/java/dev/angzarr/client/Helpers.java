package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.Message;
import com.google.protobuf.Timestamp;
import dev.angzarr.Cover;
import dev.angzarr.Edition;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.UUID;

import java.nio.ByteBuffer;
import java.time.Instant;
import java.util.Arrays;
import java.util.List;
import java.util.stream.Collectors;

/**
 * Helper methods for working with Angzarr types.
 */
public final class Helpers {

    private Helpers() {}

    /**
     * Convert a java.util.UUID to an Angzarr UUID proto.
     */
    public static UUID uuidToProto(java.util.UUID uuid) {
        ByteBuffer bb = ByteBuffer.wrap(new byte[16]);
        bb.putLong(uuid.getMostSignificantBits());
        bb.putLong(uuid.getLeastSignificantBits());
        return UUID.newBuilder()
            .setValue(ByteString.copyFrom(bb.array()))
            .build();
    }

    /**
     * Convert an Angzarr UUID proto to a java.util.UUID.
     */
    public static java.util.UUID protoToUuid(UUID uuid) {
        ByteBuffer bb = ByteBuffer.wrap(uuid.getValue().toByteArray());
        long msb = bb.getLong();
        long lsb = bb.getLong();
        return new java.util.UUID(msb, lsb);
    }

    /**
     * Get the domain from an EventBook.
     */
    public static String domain(EventBook book) {
        return book.hasCover() ? book.getCover().getDomain() : "";
    }

    /**
     * Get the correlation ID from an EventBook.
     */
    public static String correlationId(EventBook book) {
        return book.hasCover() ? book.getCover().getCorrelationId() : "";
    }

    /**
     * Check if an EventBook has a correlation ID.
     */
    public static boolean hasCorrelationId(EventBook book) {
        return book.hasCover() && !book.getCover().getCorrelationId().isEmpty();
    }

    /**
     * Get the root UUID from an EventBook.
     */
    public static UUID rootUuid(EventBook book) {
        return book.hasCover() ? book.getCover().getRoot() : null;
    }

    /**
     * Get the root UUID as hex string from an EventBook.
     */
    public static String rootIdHex(EventBook book) {
        if (!book.hasCover() || !book.getCover().hasRoot()) return "";
        byte[] bytes = book.getCover().getRoot().getValue().toByteArray();
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }

    /**
     * Get the edition from an EventBook.
     */
    public static Edition edition(EventBook book) {
        return book.hasCover() ? book.getCover().getEdition() : null;
    }

    /**
     * Calculate the next sequence number from an EventBook.
     */
    public static int nextSequence(EventBook book) {
        if (book == null || book.getPagesList().isEmpty()) {
            return 0;
        }
        return book.getPagesList().size();
    }

    /**
     * Get the type URL for a protobuf message.
     */
    public static String typeUrl(Message message) {
        return "type.googleapis.com/" + message.getDescriptorForType().getFullName();
    }

    /**
     * Extract the type name from a type URL.
     */
    public static String typeNameFromUrl(String typeUrl) {
        int idx = typeUrl.lastIndexOf('/');
        return idx >= 0 ? typeUrl.substring(idx + 1) : typeUrl;
    }

    private static final String TYPE_URL_PREFIX = "type.googleapis.com/";

    /**
     * Check if a type URL matches the given fully qualified type name.
     * @param typeUrl Full type URL (e.g., "type.googleapis.com/examples.CardsDealt")
     * @param typeName Fully qualified type name (e.g., "examples.CardsDealt")
     * @return true if typeUrl equals TYPE_URL_PREFIX + typeName
     */
    public static boolean typeUrlMatches(String typeUrl, String typeName) {
        return typeUrl.equals(TYPE_URL_PREFIX + typeName);
    }

    /**
     * Get the current timestamp as a protobuf Timestamp.
     */
    public static Timestamp now() {
        Instant now = Instant.now();
        return Timestamp.newBuilder()
            .setSeconds(now.getEpochSecond())
            .setNanos(now.getNano())
            .build();
    }

    /**
     * Pack a protobuf message into an Any.
     */
    public static Any packAny(Message message) {
        return Any.pack(message, "type.googleapis.com/");
    }

    /**
     * Pack an event into an EventPage.
     */
    public static EventPage packEvent(Message eventMessage) {
        return EventPage.newBuilder()
            .setEvent(packAny(eventMessage))
            .build();
    }

    /**
     * Pack multiple events into EventPages.
     */
    public static List<EventPage> packEvents(Message... events) {
        return Arrays.stream(events)
            .map(Helpers::packEvent)
            .collect(Collectors.toList());
    }

    /**
     * Create a new EventBook with the given events.
     */
    public static EventBook newEventBook(Message... events) {
        return EventBook.newBuilder()
            .addAllPages(packEvents(events))
            .build();
    }

    /**
     * Create a new EventBook with multiple events.
     */
    public static EventBook newEventBookMulti(List<Message> events) {
        return EventBook.newBuilder()
            .addAllPages(events.stream().map(Helpers::packEvent).collect(Collectors.toList()))
            .build();
    }
}
