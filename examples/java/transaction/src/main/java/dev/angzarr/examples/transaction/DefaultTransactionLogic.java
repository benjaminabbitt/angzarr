package dev.angzarr.examples.transaction;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Timestamp;
import examples.Domains.*;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.time.Instant;
import java.util.ArrayList;
import java.util.List;

import static net.logstash.logback.argument.StructuredArguments.kv;

/**
 * Default implementation of transaction business logic.
 */
public class DefaultTransactionLogic implements TransactionLogic {
    private static final Logger logger = LoggerFactory.getLogger(DefaultTransactionLogic.class);

    @Override
    public TransactionState rebuildState(EventBook eventBook) {
        TransactionState state = TransactionState.empty();

        if (eventBook == null || eventBook.getPagesList().isEmpty()) {
            return state;
        }

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) {
                continue;
            }
            state = applyEvent(state, page.getEvent());
        }

        return state;
    }

    private TransactionState applyEvent(TransactionState state, Any event) {
        String typeUrl = event.getTypeUrl();

        try {
            if (typeUrl.endsWith("TransactionCreated")) {
                TransactionCreated created = event.unpack(TransactionCreated.class);
                return new TransactionState(
                    created.getCustomerId(),
                    new ArrayList<>(created.getItemsList()),
                    created.getSubtotalCents(),
                    0,
                    "",
                    TransactionState.Status.PENDING
                );
            } else if (typeUrl.endsWith("DiscountApplied")) {
                DiscountApplied applied = event.unpack(DiscountApplied.class);
                return new TransactionState(
                    state.customerId(),
                    state.items(),
                    state.subtotalCents(),
                    applied.getDiscountCents(),
                    applied.getDiscountType(),
                    state.status()
                );
            } else if (typeUrl.endsWith("TransactionCompleted")) {
                return new TransactionState(
                    state.customerId(),
                    state.items(),
                    state.subtotalCents(),
                    state.discountCents(),
                    state.discountType(),
                    TransactionState.Status.COMPLETED
                );
            } else if (typeUrl.endsWith("TransactionCancelled")) {
                return new TransactionState(
                    state.customerId(),
                    state.items(),
                    state.subtotalCents(),
                    state.discountCents(),
                    state.discountType(),
                    TransactionState.Status.CANCELLED
                );
            }
        } catch (InvalidProtocolBufferException e) {
            logger.warn("Failed to unpack event: {}", typeUrl, e);
        }

        return state;
    }

    @Override
    public EventBook handleCreateTransaction(TransactionState state, String customerId, List<LineItem> items)
            throws CommandValidationException {
        if (!state.isNew()) {
            throw CommandValidationException.failedPrecondition("Transaction already exists");
        }

        if (customerId == null || customerId.isBlank()) {
            throw CommandValidationException.invalidArgument("customer_id is required");
        }

        if (items == null || items.isEmpty()) {
            throw CommandValidationException.invalidArgument("at least one item is required");
        }

        int subtotal = items.stream()
            .mapToInt(item -> item.getQuantity() * item.getUnitPriceCents())
            .sum();

        logger.info("creating_transaction",
            kv("customer_id", customerId),
            kv("item_count", items.size()),
            kv("subtotal_cents", subtotal));

        TransactionCreated event = TransactionCreated.newBuilder()
            .setCustomerId(customerId)
            .addAllItems(items)
            .setSubtotalCents(subtotal)
            .setCreatedAt(nowTimestamp())
            .build();

        return createEventBook(event);
    }

    @Override
    public EventBook handleApplyDiscount(TransactionState state, String discountType, int value, String couponCode)
            throws CommandValidationException {
        if (!state.isPending()) {
            throw CommandValidationException.failedPrecondition("Can only apply discount to pending transaction");
        }

        int discountCents = switch (discountType) {
            case "percentage" -> {
                if (value < 0 || value > 100) {
                    throw CommandValidationException.invalidArgument("Percentage must be 0-100");
                }
                yield (state.subtotalCents() * value) / 100;
            }
            case "fixed" -> Math.min(value, state.subtotalCents());
            case "coupon" -> 500; // $5 off
            default -> throw CommandValidationException.invalidArgument("Unknown discount type: " + discountType);
        };

        logger.info("applying_discount",
            kv("discount_type", discountType),
            kv("value", value),
            kv("discount_cents", discountCents));

        DiscountApplied event = DiscountApplied.newBuilder()
            .setDiscountType(discountType)
            .setValue(value)
            .setDiscountCents(discountCents)
            .setCouponCode(couponCode != null ? couponCode : "")
            .build();

        return createEventBook(event);
    }

    @Override
    public EventBook handleCompleteTransaction(TransactionState state, String paymentMethod)
            throws CommandValidationException {
        if (!state.isPending()) {
            throw CommandValidationException.failedPrecondition("Can only complete pending transaction");
        }

        int finalTotal = state.calculateFinalTotal();
        int loyaltyPoints = state.calculateLoyaltyPoints();

        logger.info("completing_transaction",
            kv("final_total_cents", finalTotal),
            kv("payment_method", paymentMethod),
            kv("loyalty_points_earned", loyaltyPoints));

        TransactionCompleted event = TransactionCompleted.newBuilder()
            .setFinalTotalCents(finalTotal)
            .setPaymentMethod(paymentMethod)
            .setLoyaltyPointsEarned(loyaltyPoints)
            .setCompletedAt(nowTimestamp())
            .build();

        return createEventBook(event);
    }

    @Override
    public EventBook handleCancelTransaction(TransactionState state, String reason)
            throws CommandValidationException {
        if (!state.isPending()) {
            throw CommandValidationException.failedPrecondition("Can only cancel pending transaction");
        }

        logger.info("cancelling_transaction", kv("reason", reason));

        TransactionCancelled event = TransactionCancelled.newBuilder()
            .setReason(reason)
            .setCancelledAt(nowTimestamp())
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
