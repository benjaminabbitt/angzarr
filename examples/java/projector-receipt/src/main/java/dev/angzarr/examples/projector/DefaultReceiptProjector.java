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
 * Default implementation of receipt projector.
 */
public class DefaultReceiptProjector implements ReceiptProjector {
    private static final Logger logger = LoggerFactory.getLogger(DefaultReceiptProjector.class);
    private static final String PROJECTOR_NAME = "receipt";

    @Override
    public Projection project(EventBook eventBook) {
        if (eventBook == null || eventBook.getPagesList().isEmpty()) {
            return null;
        }

        TransactionProjectionState state = rebuildState(eventBook);

        if (!state.completed) {
            return null;
        }

        String transactionId = "";
        if (eventBook.hasCover() && eventBook.getCover().hasRoot()) {
            transactionId = HexFormat.of().formatHex(
                eventBook.getCover().getRoot().getValue().toByteArray()
            );
        }

        String shortId = transactionId.length() > 16 ? transactionId.substring(0, 16) : transactionId;

        String receiptText = formatReceipt(transactionId, state);

        logger.info("generated_receipt",
            kv("transaction_id", shortId),
            kv("total_cents", state.finalTotalCents),
            kv("payment_method", state.paymentMethod));

        Receipt receipt = Receipt.newBuilder()
            .setTransactionId(transactionId)
            .setCustomerId(state.customerId)
            .addAllItems(state.items)
            .setSubtotalCents(state.subtotalCents)
            .setDiscountCents(state.discountCents)
            .setFinalTotalCents(state.finalTotalCents)
            .setPaymentMethod(state.paymentMethod)
            .setLoyaltyPointsEarned(state.loyaltyPointsEarned)
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

    private TransactionProjectionState rebuildState(EventBook eventBook) {
        TransactionProjectionState state = new TransactionProjectionState();

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) {
                continue;
            }

            Any event = page.getEvent();
            String typeUrl = event.getTypeUrl();

            try {
                if (typeUrl.endsWith("TransactionCreated")) {
                    TransactionCreated created = event.unpack(TransactionCreated.class);
                    state.customerId = created.getCustomerId();
                    state.items = new ArrayList<>(created.getItemsList());
                    state.subtotalCents = created.getSubtotalCents();
                } else if (typeUrl.endsWith("DiscountApplied")) {
                    DiscountApplied applied = event.unpack(DiscountApplied.class);
                    state.discountType = applied.getDiscountType();
                    state.discountCents = applied.getDiscountCents();
                } else if (typeUrl.endsWith("TransactionCompleted")) {
                    TransactionCompleted completed = event.unpack(TransactionCompleted.class);
                    state.finalTotalCents = completed.getFinalTotalCents();
                    state.paymentMethod = completed.getPaymentMethod();
                    state.loyaltyPointsEarned = completed.getLoyaltyPointsEarned();
                    state.completed = true;
                }
            } catch (InvalidProtocolBufferException e) {
                logger.warn("Failed to unpack event: {}", typeUrl, e);
            }
        }

        return state;
    }

    private String formatReceipt(String transactionId, TransactionProjectionState state) {
        StringBuilder sb = new StringBuilder();
        String line = "═".repeat(40);
        String thinLine = "─".repeat(40);

        String shortTxId = transactionId.length() > 16 ? transactionId.substring(0, 16) : transactionId;
        String shortCustId = state.customerId.length() > 16 ? state.customerId.substring(0, 16) : state.customerId;

        sb.append(line).append("\n");
        sb.append("           RECEIPT\n");
        sb.append(line).append("\n");
        sb.append("Transaction: ").append(shortTxId).append("...\n");
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
            sb.append(String.format("Discount (%s):       -$%.2f%n",
                state.discountType, state.discountCents / 100.0));
        }

        sb.append(thinLine).append("\n");
        sb.append(String.format("TOTAL:                 $%.2f%n", state.finalTotalCents / 100.0));
        sb.append("Payment: ").append(state.paymentMethod).append("\n");
        sb.append(thinLine).append("\n");
        sb.append("Loyalty Points Earned: ").append(state.loyaltyPointsEarned).append("\n");
        sb.append(line).append("\n");
        sb.append("     Thank you for your purchase!\n");
        sb.append(line);

        return sb.toString();
    }

    private static class TransactionProjectionState {
        String customerId = "";
        List<LineItem> items = new ArrayList<>();
        int subtotalCents = 0;
        int discountCents = 0;
        String discountType = "";
        int finalTotalCents = 0;
        String paymentMethod = "";
        int loyaltyPointsEarned = 0;
        boolean completed = false;
    }
}
