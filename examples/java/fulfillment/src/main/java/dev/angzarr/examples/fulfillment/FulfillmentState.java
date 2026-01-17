package dev.angzarr.examples.fulfillment;

public record FulfillmentState(
    String orderId,
    String status,
    String trackingNumber,
    String carrier,
    String pickerId,
    String packerId,
    String signature
) {
    public static FulfillmentState empty() {
        return new FulfillmentState("", "", "", "", "", "", "");
    }

    public boolean exists() {
        return !orderId.isEmpty();
    }
}
