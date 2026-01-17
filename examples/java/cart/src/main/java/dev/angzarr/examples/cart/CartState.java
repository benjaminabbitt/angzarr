package dev.angzarr.examples.cart;

import java.util.HashMap;
import java.util.Map;

public record CartState(
    String customerId,
    Map<String, CartItem> items,
    int subtotalCents,
    String couponCode,
    int discountCents,
    String status
) {
    public static CartState empty() {
        return new CartState("", new HashMap<>(), 0, "", 0, "");
    }

    public boolean exists() {
        return !customerId.isEmpty();
    }

    public boolean isCheckedOut() {
        return "checked_out".equals(status);
    }

    public int calculateSubtotal() {
        return items.values().stream()
            .mapToInt(item -> item.quantity() * item.unitPriceCents())
            .sum();
    }

    public record CartItem(String productId, String name, int quantity, int unitPriceCents) {}
}
