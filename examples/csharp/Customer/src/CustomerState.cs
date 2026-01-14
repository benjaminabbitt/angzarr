namespace Angzarr.Examples.Customer;

/// <summary>
/// Represents the current state of a customer aggregate.
/// </summary>
public record CustomerState(
    string Name,
    string Email,
    int LoyaltyPoints,
    int LifetimePoints)
{
    public static CustomerState Empty => new("", "", 0, 0);

    public bool Exists => !string.IsNullOrEmpty(Name);

    public CustomerState WithName(string name) =>
        this with { Name = name };

    public CustomerState WithEmail(string email) =>
        this with { Email = email };

    public CustomerState WithLoyaltyPoints(int points) =>
        this with { LoyaltyPoints = points };

    public CustomerState AddLifetimePoints(int points) =>
        this with { LifetimePoints = LifetimePoints + points };
}
