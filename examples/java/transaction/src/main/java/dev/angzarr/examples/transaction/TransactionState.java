package dev.angzarr.examples.transaction;

import examples.Domains.LineItem;
import java.util.List;

/**
 * Represents the current state of a transaction aggregate.
 */
public record TransactionState(
    String customerId,
    List<LineItem> items,
    int subtotalCents,
    int discountCents,
    String discountType,
    Status status
) {
    public enum Status {
        NEW, PENDING, COMPLETED, CANCELLED
    }

    public static TransactionState empty() {
        return new TransactionState("", List.of(), 0, 0, "", Status.NEW);
    }

    public boolean isPending() {
        return status == Status.PENDING;
    }

    public boolean isNew() {
        return status == Status.NEW;
    }

    public int calculateFinalTotal() {
        int total = subtotalCents - discountCents;
        return Math.max(0, total);
    }

    public int calculateLoyaltyPoints() {
        return calculateFinalTotal() / 100; // 1 point per dollar
    }
}
