package dev.angzarr.examples.inventory;

import java.util.HashMap;
import java.util.Map;

public record InventoryState(
    String productId,
    int onHand,
    int reserved,
    int lowStockThreshold,
    Map<String, Integer> reservations
) {
    public static InventoryState empty() {
        return new InventoryState("", 0, 0, 0, new HashMap<>());
    }

    public boolean exists() {
        return !productId.isEmpty();
    }

    public int available() {
        return onHand - reserved;
    }
}
