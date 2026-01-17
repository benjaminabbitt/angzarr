package dev.angzarr.examples.order;

import java.util.List;
import java.util.ArrayList;

public record OrderState(
    String customerId,
    List<LineItem> items,
    int subtotalCents,
    int discountCents,
    int loyaltyPointsUsed,
    String paymentMethod,
    String paymentReference,
    String status
) {
    public static OrderState empty() {
        return new OrderState("", new ArrayList<>(), 0, 0, 0, "", "", "");
    }

    public boolean exists() {
        return !customerId.isEmpty();
    }

    public boolean isPending() {
        return "pending".equals(status);
    }

    public int totalCents() {
        return subtotalCents - discountCents;
    }

    public record LineItem(String productId, String name, int quantity, int unitPriceCents) {}
}
