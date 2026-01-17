namespace Angzarr.Examples.Inventory;

public record Reservation(string OrderId, int Quantity);

public record InventoryState(
    string ProductId,
    int OnHand,
    int Reserved,
    List<Reservation> Reservations,
    int LowStockThreshold)
{
    public static InventoryState Empty => new("", 0, 0, new List<Reservation>(), 10);

    public bool Exists => !string.IsNullOrEmpty(ProductId);
    public int Available => OnHand - Reserved;
}
