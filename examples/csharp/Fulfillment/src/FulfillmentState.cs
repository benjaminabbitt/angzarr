namespace Angzarr.Examples.Fulfillment;

public record FulfillmentState(
    string OrderId,
    string Status,
    string TrackingNumber)
{
    public static FulfillmentState Empty => new("", "", "");

    public bool Exists => !string.IsNullOrEmpty(OrderId);
    public bool IsPending => Status == "pending";
    public bool IsPicking => Status == "picking";
    public bool IsPacking => Status == "packing";
    public bool IsShipped => Status == "shipped";
    public bool IsDelivered => Status == "delivered";
}
