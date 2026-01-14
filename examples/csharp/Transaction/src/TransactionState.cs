using Examples;

namespace Angzarr.Examples.Transaction;

public enum TransactionStatus
{
    New,
    Pending,
    Completed,
    Cancelled
}

/// <summary>
/// Represents the current state of a transaction aggregate.
/// </summary>
public record TransactionState(
    string CustomerId,
    IReadOnlyList<LineItem> Items,
    int SubtotalCents,
    int DiscountCents,
    string DiscountType,
    TransactionStatus Status)
{
    public static TransactionState Empty => new("", Array.Empty<LineItem>(), 0, 0, "", TransactionStatus.New);

    public bool IsPending => Status == TransactionStatus.Pending;
    public bool IsNew => Status == TransactionStatus.New;

    public int CalculateFinalTotal() => Math.Max(0, SubtotalCents - DiscountCents);
    public int CalculateLoyaltyPoints() => CalculateFinalTotal() / 100;
}
