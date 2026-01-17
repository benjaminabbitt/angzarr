using Examples;

namespace Angzarr.Examples.Order;

public record OrderState(
    string CustomerId,
    List<LineItem> Items,
    int SubtotalCents,
    int DiscountCents,
    int LoyaltyPointsUsed,
    int FinalTotalCents,
    string PaymentMethod,
    string Status)
{
    public static OrderState Empty => new("", new List<LineItem>(), 0, 0, 0, 0, "", "");

    public bool Exists => !string.IsNullOrEmpty(CustomerId);
    public bool IsPendingPayment => Status == "pending_payment";
    public bool IsPaid => Status == "paid";
    public bool IsCompleted => Status == "completed";
    public bool IsCancelled => Status == "cancelled";
}
