package dev.angzarr.examples.product;

public record ProductState(
    String sku,
    String name,
    String description,
    int priceCents,
    String status
) {
    public static ProductState empty() {
        return new ProductState("", "", "", 0, "");
    }

    public boolean exists() {
        return !sku.isEmpty();
    }

    public boolean isActive() {
        return "active".equals(status);
    }

    public boolean isDiscontinued() {
        return "discontinued".equals(status);
    }
}
