package dev.angzarr.examples.customer;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Timestamp;
import examples.Domains.CustomerCreated;
import examples.Domains.LoyaltyPointsAdded;
import examples.Domains.LoyaltyPointsRedeemed;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.Snapshot;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import static net.logstash.logback.argument.StructuredArguments.kv;

import java.time.Instant;
import java.util.List;

/**
 * Default implementation of customer business logic.
 */
public class DefaultCustomerLogic implements CustomerLogic {
    private static final Logger logger = LoggerFactory.getLogger(DefaultCustomerLogic.class);

    @Override
    public CustomerState rebuildState(EventBook eventBook) {
        CustomerState state = CustomerState.empty();

        if (eventBook == null || eventBook.getPagesList().isEmpty()) {
            return state;
        }

        // Start from snapshot if present
        if (eventBook.hasSnapshot() && eventBook.getSnapshot().hasState()) {
            try {
                var snapState = eventBook.getSnapshot().getState()
                    .unpack(examples.Domains.CustomerState.class);
                state = new CustomerState(
                    snapState.getName(),
                    snapState.getEmail(),
                    snapState.getLoyaltyPoints(),
                    snapState.getLifetimePoints()
                );
            } catch (InvalidProtocolBufferException e) {
                logger.warn("Failed to unpack snapshot state", e);
            }
        }

        // Apply events
        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) {
                continue;
            }

            Any event = page.getEvent();
            state = applyEvent(state, event);
        }

        return state;
    }

    private CustomerState applyEvent(CustomerState state, Any event) {
        String typeUrl = event.getTypeUrl();

        try {
            if (typeUrl.endsWith("CustomerCreated")) {
                CustomerCreated created = event.unpack(CustomerCreated.class);
                return new CustomerState(created.getName(), created.getEmail(), 0, 0);
            } else if (typeUrl.endsWith("LoyaltyPointsAdded")) {
                LoyaltyPointsAdded added = event.unpack(LoyaltyPointsAdded.class);
                return state.withLoyaltyPoints(added.getNewBalance())
                    .addLifetimePoints(added.getPoints());
            } else if (typeUrl.endsWith("LoyaltyPointsRedeemed")) {
                LoyaltyPointsRedeemed redeemed = event.unpack(LoyaltyPointsRedeemed.class);
                return state.withLoyaltyPoints(redeemed.getNewBalance());
            }
        } catch (InvalidProtocolBufferException e) {
            logger.warn("Failed to unpack event: {}", typeUrl, e);
        }

        return state;
    }

    @Override
    public EventBook handleCreateCustomer(CustomerState state, String name, String email)
            throws CommandValidationException {
        if (state.exists()) {
            throw CommandValidationException.failedPrecondition("Customer already exists");
        }

        if (name == null || name.isBlank()) {
            throw CommandValidationException.invalidArgument("Customer name is required");
        }

        if (email == null || email.isBlank()) {
            throw CommandValidationException.invalidArgument("Customer email is required");
        }

        logger.info("creating_customer", kv("name", name), kv("email", email));

        CustomerCreated event = CustomerCreated.newBuilder()
            .setName(name)
            .setEmail(email)
            .setCreatedAt(nowTimestamp())
            .build();

        return createEventBook(event);
    }

    @Override
    public EventBook handleAddLoyaltyPoints(CustomerState state, int points, String reason)
            throws CommandValidationException {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Customer does not exist");
        }

        if (points <= 0) {
            throw CommandValidationException.invalidArgument("Points must be positive");
        }

        int newBalance = state.loyaltyPoints() + points;

        logger.info("adding_loyalty_points",
            kv("points", points), kv("new_balance", newBalance), kv("reason", reason));

        LoyaltyPointsAdded event = LoyaltyPointsAdded.newBuilder()
            .setPoints(points)
            .setNewBalance(newBalance)
            .setReason(reason != null ? reason : "")
            .build();

        return createEventBook(event);
    }

    @Override
    public EventBook handleRedeemLoyaltyPoints(CustomerState state, int points, String redemptionType)
            throws CommandValidationException {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Customer does not exist");
        }

        if (points <= 0) {
            throw CommandValidationException.invalidArgument("Points must be positive");
        }

        if (points > state.loyaltyPoints()) {
            throw CommandValidationException.failedPrecondition(
                String.format("Insufficient points: have %d, need %d",
                    state.loyaltyPoints(), points));
        }

        int newBalance = state.loyaltyPoints() - points;

        logger.info("redeeming_loyalty_points",
            kv("points", points), kv("new_balance", newBalance), kv("redemption_type", redemptionType));

        LoyaltyPointsRedeemed event = LoyaltyPointsRedeemed.newBuilder()
            .setPoints(points)
            .setNewBalance(newBalance)
            .setRedemptionType(redemptionType != null ? redemptionType : "")
            .build();

        return createEventBook(event);
    }

    private EventBook createEventBook(com.google.protobuf.Message event) {
        EventPage page = EventPage.newBuilder()
            .setNum(0)
            .setEvent(Any.pack(event))
            .setCreatedAt(nowTimestamp())
            .build();

        return EventBook.newBuilder()
            .addPages(page)
            .build();
    }

    private Timestamp nowTimestamp() {
        Instant now = Instant.now();
        return Timestamp.newBuilder()
            .setSeconds(now.getEpochSecond())
            .setNanos(now.getNano())
            .build();
    }
}
