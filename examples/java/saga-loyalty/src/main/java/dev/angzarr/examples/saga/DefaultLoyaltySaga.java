package dev.angzarr.examples.saga;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import examples.Domains.AddLoyaltyPoints;
import examples.Domains.TransactionCompleted;
import dev.angzarr.*;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;

import static net.logstash.logback.argument.StructuredArguments.kv;

/**
 * Default implementation of loyalty points saga.
 * Listens to TransactionCompleted events and awards loyalty points.
 */
public class DefaultLoyaltySaga implements LoyaltySaga {
    private static final Logger logger = LoggerFactory.getLogger(DefaultLoyaltySaga.class);

    @Override
    public List<CommandBook> processEvents(EventBook eventBook) {
        if (eventBook == null || eventBook.getPagesList().isEmpty()) {
            return List.of();
        }

        List<CommandBook> commands = new ArrayList<>();

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) {
                continue;
            }

            Any event = page.getEvent();
            if (!event.getTypeUrl().endsWith("TransactionCompleted")) {
                continue;
            }

            try {
                TransactionCompleted completed = event.unpack(TransactionCompleted.class);
                int points = completed.getLoyaltyPointsEarned();

                if (points <= 0) {
                    continue;
                }

                UUID customerId = null;
                String transactionId = "";

                if (eventBook.hasCover() && eventBook.getCover().hasRoot()) {
                    customerId = eventBook.getCover().getRoot();
                    transactionId = HexFormat.of().formatHex(customerId.getValue().toByteArray());
                }

                if (customerId == null) {
                    logger.warn("Transaction has no root ID, skipping loyalty points");
                    continue;
                }

                String shortId = transactionId.length() > 16 ? transactionId.substring(0, 16) : transactionId;

                logger.info("awarding_loyalty_points",
                    kv("points", points),
                    kv("transaction_id", shortId));

                AddLoyaltyPoints addPoints = AddLoyaltyPoints.newBuilder()
                    .setPoints(points)
                    .setReason("transaction:" + transactionId)
                    .build();

                CommandBook commandBook = CommandBook.newBuilder()
                    .setCover(Cover.newBuilder()
                        .setDomain("customer")
                        .setRoot(customerId)
                        .build())
                    .addPages(CommandPage.newBuilder()
                        .setSequence(0)
                        .setSynchronous(false)
                        .setCommand(Any.pack(addPoints))
                        .build())
                    .build();

                commands.add(commandBook);

            } catch (InvalidProtocolBufferException e) {
                logger.error("Failed to unpack TransactionCompleted event", e);
            }
        }

        return commands;
    }
}
