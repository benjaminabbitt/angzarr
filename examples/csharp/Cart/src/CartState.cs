using Examples;

namespace Angzarr.Examples.Cart;

public record CartState(
    string CustomerId,
    List<LineItem> Items,
    int SubtotalCents,
    string CouponCode,
    int DiscountCents,
    string Status)
{
    public static CartState Empty => new("", new List<LineItem>(), 0, "", 0, "");

    public bool Exists => !string.IsNullOrEmpty(CustomerId);
    public bool IsActive => Status == "active";
    public bool IsCheckedOut => Status == "checked_out";
}
