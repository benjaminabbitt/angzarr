package dev.angzarr.examples.projector;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import examples.Domains.*;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.Projection;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;

import static net.logstash.logback.argument.StructuredArguments.kv;

/**
 * Default implementation of receipt projector for Order domain.
 */
public class DefaultReceiptProjector implements ReceiptProjector {
    private static final Logger logger = LoggerFactory.getLogger(DefaultReceiptProjector.class);
    private static final String PROJECTOR_NAME = "receipt";
    private static final int POINTS_PER_DOLLAR = 10;

    @Override
    public Projection project(EventBook eventBook) {
        if (eventBook == null || eventBook.getPagesList().isEmpty()) {
            return null;
        }

        OrderProjectionState state = rebuildState(eventBook);

        if (!state.completed) {
            return null;
        }

        String orderId = "";
        if (eventBook.hasCover() && eventBook.getCover().hasRoot()) {
            orderId = HexFormat.of().formatHex(
                eventBook.getCover().getRoot().getValue().toByteArray()
            );
        }

        String shortId = orderId.length() > 16 ? orderId.substring(0, 16) : orderId;

        int loyaltyPointsEarned = (state.finalTotalCents / 100) * POINTS_PER_DOLLAR;
        String receiptText = formatReceipt(orderId, state, loyaltyPointsEarned);

        logger.info("generated_receipt",
            kv("order_id", shortId),
            kv("total_cents", state.finalTotalCents),
            kv("payment_method", state.paymentMethod));

        Receipt receipt = Receipt.newBuilder()
            .setOrderId(orderId)
            .setCustomerId(state.customerId)
            .addAllItems(state.items)
            .setSubtotalCents(state.subtotalCents)
            .setDiscountCents(state.discountCents)
            .setFinalTotalCents(state.finalTotalCents)
            .setPaymentMethod(state.paymentMethod)
            .setLoyaltyPointsEarned(loyaltyPointsEarned)
            .setFormattedText(receiptText)
            .build();

        int sequence = 0;
        if (!eventBook.getPagesList().isEmpty()) {
            EventPage lastPage = eventBook.getPages(eventBook.getPagesCount() - 1);
            if (lastPage.hasNum()) {
                sequence = lastPage.getNum();
            }
        }

        return Projection.newBuilder()
            .setCover(eventBook.getCover())
            .setProjector(PROJECTOR_NAME)
            .setSequence(sequence)
            .setProjection(Any.pack(receipt))
            .build();
    }

    private OrderProjectionState rebuildState(EventBook eventBook) {
        OrderProjectionState state = new OrderProjectionState();

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) {
                continue;
            }

            Any event = page.getEvent();
            String typeUrl = event.getTypeUrl();

            try {
                if (typeUrl.endsWith("OrderCreated")) {
                    OrderCreated created = event.unpack(OrderCreated.class);
                    state.customerId = created.getCustomerId();
                    state.items = new ArrayList<>(created.getItemsList());
                    state.subtotalCents = created.getSubtotalCents();
                    state.discountCents = created.getDiscountCents();
                } else if (typeUrl.endsWith("LoyaltyDiscountApplied")) {
                    LoyaltyDiscountApplied applied = event.unpack(LoyaltyDiscountApplied.class);
                    state.loyaltyPointsUsed = applied.getPointsUsed();
                    state.discountCents += applied.getDiscountCents();
                } else if (typeUrl.endsWith("PaymentSubmitted")) {
                    PaymentSubmitted submitted = event.unpack(PaymentSubmitted.class);
                    state.paymentMethod = submitted.getPaymentMethod();
                    state.finalTotalCents = submitted.getAmountCents();
                } else if (typeUrl.endsWith("OrderCompleted")) {
                    state.completed = true;
                }
            } catch (InvalidProtocolBufferException e) {
                logger.warn("Failed to unpack event: {}", typeUrl, e);
            }
        }

        return state;
    }

    private String formatReceipt(String orderId, OrderProjectionState state, int loyaltyPointsEarned) {
        StringBuilder sb = new StringBuilder();
        String line = "═".repeat(40);
        String thinLine = "─".repeat(40);

        String shortOrderId = orderId.length() > 16 ? orderId.substring(0, 16) : orderId;
        String shortCustId = state.customerId.length() > 16 ? state.customerId.substring(0, 16) : state.customerId;

        sb.append(line).append("\n");
        sb.append("           RECEIPT\n");
        sb.append(line).append("\n");
        sb.append("Order: ").append(shortOrderId).append("...\n");
        sb.append("Customer: ").append(shortCustId.isEmpty() ? "N/A" : shortCustId + "...").append("\n");
        sb.append(thinLine).append("\n");

        for (LineItem item : state.items) {
            int lineTotal = item.getQuantity() * item.getUnitPriceCents();
            sb.append(String.format("%d x %s @ $%.2f = $%.2f%n",
                item.getQuantity(),
                item.getName(),
                item.getUnitPriceCents() / 100.0,
                lineTotal / 100.0));
        }

        sb.append(thinLine).append("\n");
        sb.append(String.format("Subtotal:              $%.2f%n", state.subtotalCents / 100.0));

        if (state.discountCents > 0) {
            String discountType = state.loyaltyPointsUsed > 0 ? "loyalty" : "coupon";
            sb.append(String.format("Discount (%s):       -$%.2f%n",
                discountType, state.discountCents / 100.0));
        }

        sb.append(thinLine).append("\n");
        sb.append(String.format("TOTAL:                 $%.2f%n", state.finalTotalCents / 100.0));
        sb.append("Payment: ").append(state.paymentMethod).append("\n");
        sb.append(thinLine).append("\n");
        sb.append("Loyalty Points Earned: ").append(loyaltyPointsEarned).append("\n");
        sb.append(line).append("\n");
        sb.append("     Thank you for your purchase!\n");
        sb.append(line);

        return sb.toString();
    }

    private static class OrderProjectionState {
        String customerId = "";
        List<LineItem> items = new ArrayList<>();
        int subtotalCents = 0;
        int discountCents = 0;
        int loyaltyPointsUsed = 0;
        int finalTotalCents = 0;
        String paymentMethod = "";
        boolean completed = false;
    }
}
