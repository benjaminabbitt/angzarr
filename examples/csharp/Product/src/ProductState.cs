namespace Angzarr.Examples.Product;

public record ProductState(
    string Sku,
    string Name,
    string Description,
    int PriceCents,
    string Status)
{
    public static ProductState Empty => new("", "", "", 0, "");

    public bool Exists => !string.IsNullOrEmpty(Sku);
    public bool IsActive => Status == "active";
    public bool IsDiscontinued => Status == "discontinued";
}
