using Angzarr;

namespace Angzarr.Examples.Customer;

/// <summary>
/// Interface for customer business logic operations.
/// </summary>
public interface ICustomerLogic
{
    CustomerState RebuildState(EventBook? eventBook);

    EventBook HandleCreateCustomer(CustomerState state, string name, string email);

    EventBook HandleAddLoyaltyPoints(CustomerState state, int points, string reason);

    EventBook HandleRedeemLoyaltyPoints(CustomerState state, int points, string redemptionType);
}
