package dev.angzarr.examples.projector;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Timestamp;
import examples.Domains.*;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.time.Instant;
import java.time.ZoneOffset;
import java.time.format.DateTimeFormatter;
import java.util.HexFormat;

import static net.logstash.logback.argument.StructuredArguments.kv;

/**
 * Default implementation of customer event logging projector.
 */
public class DefaultCustomerLogProjector implements CustomerLogProjector {
    private static final Logger logger = LoggerFactory.getLogger(DefaultCustomerLogProjector.class);

    @Override
    public void logEvents(EventBook eventBook) {
        if (eventBook == null || eventBook.getPagesList().isEmpty()) {
            return;
        }

        String domain = "customer";
        if (eventBook.hasCover()) {
            domain = eventBook.getCover().getDomain();
        }

        String rootId = "";
        if (eventBook.hasCover() && eventBook.getCover().hasRoot()) {
            rootId = HexFormat.of().formatHex(
                eventBook.getCover().getRoot().getValue().toByteArray()
            );
        }
        String shortId = rootId.length() > 16 ? rootId.substring(0, 16) : rootId;

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) {
                continue;
            }

            int sequence = page.hasNum() ? page.getNum() : 0;
            String eventType = extractEventType(page.getEvent().getTypeUrl());

            logEventDetails(domain, shortId, sequence, eventType, page.getEvent());
        }
    }

    private String extractEventType(String typeUrl) {
        int idx = typeUrl.lastIndexOf('.');
        return idx >= 0 ? typeUrl.substring(idx + 1) : typeUrl;
    }

    private void logEventDetails(String domain, String rootId, int sequence, String eventType, Any event) {
        try {
            switch (eventType) {
                case "CustomerCreated" -> {
                    CustomerCreated created = event.unpack(CustomerCreated.class);
                    logger.info("event",
                        kv("domain", domain),
                        kv("root_id", rootId),
                        kv("sequence", sequence),
                        kv("event_type", eventType),
                        kv("name", created.getName()),
                        kv("email", created.getEmail()),
                        kv("created_at", formatTimestamp(created.getCreatedAt())));
                }
                case "LoyaltyPointsAdded" -> {
                    LoyaltyPointsAdded added = event.unpack(LoyaltyPointsAdded.class);
                    logger.info("event",
                        kv("domain", domain),
                        kv("root_id", rootId),
                        kv("sequence", sequence),
                        kv("event_type", eventType),
                        kv("points", added.getPoints()),
                        kv("new_balance", added.getNewBalance()),
                        kv("reason", added.getReason()));
                }
                case "LoyaltyPointsRedeemed" -> {
                    LoyaltyPointsRedeemed redeemed = event.unpack(LoyaltyPointsRedeemed.class);
                    logger.info("event",
                        kv("domain", domain),
                        kv("root_id", rootId),
                        kv("sequence", sequence),
                        kv("event_type", eventType),
                        kv("points", redeemed.getPoints()),
                        kv("new_balance", redeemed.getNewBalance()),
                        kv("redemption_type", redeemed.getRedemptionType()));
                }
                default -> logger.info("event",
                    kv("domain", domain),
                    kv("root_id", rootId),
                    kv("sequence", sequence),
                    kv("event_type", eventType),
                    kv("raw_bytes", event.getValue().size()));
            }
        } catch (InvalidProtocolBufferException e) {
            logger.warn("Failed to unpack event",
                kv("event_type", eventType),
                kv("error", e.getMessage()));
        }
    }

    private String formatTimestamp(Timestamp ts) {
        if (ts == null) {
            return "";
        }
        return Instant.ofEpochSecond(ts.getSeconds(), ts.getNanos())
            .atOffset(ZoneOffset.UTC)
            .format(DateTimeFormatter.ISO_INSTANT);
    }
}
