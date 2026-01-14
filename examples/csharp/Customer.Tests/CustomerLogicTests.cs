using Angzarr;
using Examples;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Moq;
using Serilog;
using Xunit;

namespace Angzarr.Examples.Customer.Tests;

public class CustomerLogicTests
{
    private readonly ICustomerLogic _logic;

    public CustomerLogicTests()
    {
        var logger = new Mock<ILogger>();
        logger.Setup(l => l.ForContext<CustomerLogic>()).Returns(logger.Object);
        _logic = new CustomerLogic(logger.Object);
    }

    [Fact]
    public void RebuildState_Null_ReturnsEmptyState()
    {
        var state = _logic.RebuildState(null);

        Assert.False(state.Exists);
        Assert.Equal("", state.Name);
        Assert.Equal("", state.Email);
        Assert.Equal(0, state.LoyaltyPoints);
    }

    [Fact]
    public void RebuildState_EmptyEventBook_ReturnsEmptyState()
    {
        var eventBook = new EventBook();

        var state = _logic.RebuildState(eventBook);

        Assert.False(state.Exists);
    }

    [Fact]
    public void RebuildState_WithCustomerCreated_ReturnsState()
    {
        var evt = new CustomerCreated { Name = "John Doe", Email = "john@example.com" };
        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(evt) });

        var state = _logic.RebuildState(eventBook);

        Assert.True(state.Exists);
        Assert.Equal("John Doe", state.Name);
        Assert.Equal("john@example.com", state.Email);
    }

    [Fact]
    public void RebuildState_WithLoyaltyPointsAdded_UpdatesBalance()
    {
        var created = new CustomerCreated { Name = "John", Email = "john@test.com" };
        var added = new LoyaltyPointsAdded { Points = 100, NewBalance = 100, Reason = "welcome" };

        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });
        eventBook.Pages.Add(new EventPage { Num = 1, Event = Any.Pack(added) });

        var state = _logic.RebuildState(eventBook);

        Assert.Equal(100, state.LoyaltyPoints);
        Assert.Equal(100, state.LifetimePoints);
    }

    [Fact]
    public void HandleCreateCustomer_Success_ReturnsEventBook()
    {
        var state = CustomerState.Empty;

        var result = _logic.HandleCreateCustomer(state, "Jane Doe", "jane@example.com");

        Assert.NotNull(result);
        Assert.Single(result.Pages);
        Assert.EndsWith("CustomerCreated", result.Pages[0].Event.TypeUrl);
    }

    [Fact]
    public void HandleCreateCustomer_AlreadyExists_Throws()
    {
        var state = new CustomerState("Existing", "existing@test.com", 0, 0);

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleCreateCustomer(state, "New", "new@test.com"));

        Assert.Equal(StatusCode.FailedPrecondition, ex.StatusCode);
    }

    [Fact]
    public void HandleCreateCustomer_EmptyName_Throws()
    {
        var state = CustomerState.Empty;

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleCreateCustomer(state, "", "email@test.com"));

        Assert.Equal(StatusCode.InvalidArgument, ex.StatusCode);
    }

    [Fact]
    public void HandleAddLoyaltyPoints_Success_ReturnsEventBook()
    {
        var state = new CustomerState("John", "john@test.com", 50, 100);

        var result = _logic.HandleAddLoyaltyPoints(state, 25, "purchase");

        Assert.NotNull(result);
        Assert.Single(result.Pages);
        Assert.EndsWith("LoyaltyPointsAdded", result.Pages[0].Event.TypeUrl);
    }

    [Fact]
    public void HandleAddLoyaltyPoints_CustomerNotExists_Throws()
    {
        var state = CustomerState.Empty;

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleAddLoyaltyPoints(state, 25, "purchase"));

        Assert.Equal(StatusCode.FailedPrecondition, ex.StatusCode);
    }

    [Fact]
    public void HandleAddLoyaltyPoints_ZeroPoints_Throws()
    {
        var state = new CustomerState("John", "john@test.com", 50, 100);

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleAddLoyaltyPoints(state, 0, "purchase"));

        Assert.Equal(StatusCode.InvalidArgument, ex.StatusCode);
    }

    [Fact]
    public void HandleRedeemLoyaltyPoints_Success_ReturnsEventBook()
    {
        var state = new CustomerState("John", "john@test.com", 100, 200);

        var result = _logic.HandleRedeemLoyaltyPoints(state, 50, "discount");

        Assert.NotNull(result);
        Assert.Single(result.Pages);
        Assert.EndsWith("LoyaltyPointsRedeemed", result.Pages[0].Event.TypeUrl);
    }

    [Fact]
    public void HandleRedeemLoyaltyPoints_InsufficientPoints_Throws()
    {
        var state = new CustomerState("John", "john@test.com", 30, 100);

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleRedeemLoyaltyPoints(state, 50, "discount"));

        Assert.Equal(StatusCode.FailedPrecondition, ex.StatusCode);
        Assert.Contains("Insufficient", ex.Message);
    }
}
