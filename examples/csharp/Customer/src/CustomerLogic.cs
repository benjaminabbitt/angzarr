using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Serilog;

namespace Angzarr.Examples.Customer;

/// <summary>
/// Default implementation of customer business logic.
/// </summary>
public class CustomerLogic : ICustomerLogic
{
    private readonly Serilog.ILogger _logger;

    public CustomerLogic(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<CustomerLogic>();
    }

    public CustomerState RebuildState(EventBook? eventBook)
    {
        var state = CustomerState.Empty;

        if (eventBook == null || eventBook.Pages.Count == 0)
            return state;

        // Start from snapshot if present
        if (eventBook.Snapshot?.State != null)
        {
            if (eventBook.Snapshot.State.TryUnpack<global::Examples.CustomerState>(out var snapState))
            {
                state = new CustomerState(
                    snapState.Name,
                    snapState.Email,
                    snapState.LoyaltyPoints,
                    snapState.LifetimePoints);
            }
        }

        // Apply events
        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;
            state = ApplyEvent(state, page.Event);
        }

        return state;
    }

    private CustomerState ApplyEvent(CustomerState state, Any eventAny)
    {
        var typeUrl = eventAny.TypeUrl;

        if (typeUrl.EndsWith("CustomerCreated"))
        {
            var evt = eventAny.Unpack<CustomerCreated>();
            return new CustomerState(evt.Name, evt.Email, 0, 0);
        }

        if (typeUrl.EndsWith("LoyaltyPointsAdded"))
        {
            var evt = eventAny.Unpack<LoyaltyPointsAdded>();
            return state
                .WithLoyaltyPoints(evt.NewBalance)
                .AddLifetimePoints(evt.Points);
        }

        if (typeUrl.EndsWith("LoyaltyPointsRedeemed"))
        {
            var evt = eventAny.Unpack<LoyaltyPointsRedeemed>();
            return state.WithLoyaltyPoints(evt.NewBalance);
        }

        return state;
    }

    public EventBook HandleCreateCustomer(CustomerState state, string name, string email)
    {
        if (state.Exists)
            throw CommandValidationException.FailedPrecondition("Customer already exists");

        if (string.IsNullOrWhiteSpace(name))
            throw CommandValidationException.InvalidArgument("Customer name is required");

        if (string.IsNullOrWhiteSpace(email))
            throw CommandValidationException.InvalidArgument("Customer email is required");

        _logger.Information("creating_customer {@Data}", new { name, email });

        var evt = new CustomerCreated
        {
            Name = name,
            Email = email,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleAddLoyaltyPoints(CustomerState state, int points, string reason)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Customer does not exist");

        if (points <= 0)
            throw CommandValidationException.InvalidArgument("Points must be positive");

        var newBalance = state.LoyaltyPoints + points;

        _logger.Information("adding_loyalty_points {@Data}",
            new { points, new_balance = newBalance, reason });

        var evt = new LoyaltyPointsAdded
        {
            Points = points,
            NewBalance = newBalance,
            Reason = reason ?? ""
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleRedeemLoyaltyPoints(CustomerState state, int points, string redemptionType)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Customer does not exist");

        if (points <= 0)
            throw CommandValidationException.InvalidArgument("Points must be positive");

        if (points > state.LoyaltyPoints)
            throw CommandValidationException.FailedPrecondition(
                $"Insufficient points: have {state.LoyaltyPoints}, need {points}");

        var newBalance = state.LoyaltyPoints - points;

        _logger.Information("redeeming_loyalty_points {@Data}",
            new { points, new_balance = newBalance, redemption_type = redemptionType });

        var evt = new LoyaltyPointsRedeemed
        {
            Points = points,
            NewBalance = newBalance,
            RedemptionType = redemptionType ?? ""
        };

        return CreateEventBook(evt);
    }

    private static EventBook CreateEventBook(IMessage evt)
    {
        var page = new EventPage
        {
            Num = 0,
            Event = Any.Pack(evt),
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        var book = new EventBook();
        book.Pages.Add(page);
        return book;
    }
}
