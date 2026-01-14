using Angzarr;
using Examples;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Moq;
using Serilog;
using Xunit;

namespace Angzarr.Examples.Transaction.Tests;

public class TransactionLogicTests
{
    private readonly ITransactionLogic _logic;

    public TransactionLogicTests()
    {
        var logger = new Mock<ILogger>();
        logger.Setup(l => l.ForContext<TransactionLogic>()).Returns(logger.Object);
        _logic = new TransactionLogic(logger.Object);
    }

    [Fact]
    public void RebuildState_Null_ReturnsEmptyState()
    {
        var state = _logic.RebuildState(null);

        Assert.True(state.IsNew);
        Assert.Equal("", state.CustomerId);
        Assert.Empty(state.Items);
    }

    [Fact]
    public void RebuildState_EmptyEventBook_ReturnsEmptyState()
    {
        var eventBook = new EventBook();

        var state = _logic.RebuildState(eventBook);

        Assert.True(state.IsNew);
    }

    [Fact]
    public void RebuildState_WithTransactionCreated_ReturnsState()
    {
        var evt = new TransactionCreated
        {
            CustomerId = "cust-123",
            SubtotalCents = 1000,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.Items.Add(new LineItem { Name = "Widget", Quantity = 2, UnitPriceCents = 500 });

        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(evt) });

        var state = _logic.RebuildState(eventBook);

        Assert.True(state.IsPending);
        Assert.Equal("cust-123", state.CustomerId);
        Assert.Single(state.Items);
        Assert.Equal(1000, state.SubtotalCents);
    }

    [Fact]
    public void RebuildState_WithDiscountApplied_UpdatesDiscount()
    {
        var created = new TransactionCreated { CustomerId = "cust-1", SubtotalCents = 1000 };
        var discount = new DiscountApplied { DiscountType = "percentage", Value = 10, DiscountCents = 100 };

        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });
        eventBook.Pages.Add(new EventPage { Num = 1, Event = Any.Pack(discount) });

        var state = _logic.RebuildState(eventBook);

        Assert.Equal(100, state.DiscountCents);
        Assert.Equal("percentage", state.DiscountType);
    }

    [Fact]
    public void RebuildState_WithTransactionCompleted_StatusCompleted()
    {
        var created = new TransactionCreated { CustomerId = "cust-1", SubtotalCents = 1000 };
        var completed = new TransactionCompleted { FinalTotalCents = 1000, PaymentMethod = "card" };

        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });
        eventBook.Pages.Add(new EventPage { Num = 1, Event = Any.Pack(completed) });

        var state = _logic.RebuildState(eventBook);

        Assert.Equal(TransactionStatus.Completed, state.Status);
    }

    [Fact]
    public void RebuildState_WithTransactionCancelled_StatusCancelled()
    {
        var created = new TransactionCreated { CustomerId = "cust-1", SubtotalCents = 1000 };
        var cancelled = new TransactionCancelled { Reason = "user request" };

        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });
        eventBook.Pages.Add(new EventPage { Num = 1, Event = Any.Pack(cancelled) });

        var state = _logic.RebuildState(eventBook);

        Assert.Equal(TransactionStatus.Cancelled, state.Status);
    }

    [Fact]
    public void HandleCreateTransaction_Success_ReturnsEventBook()
    {
        var state = TransactionState.Empty;
        var items = new[] { new LineItem { Name = "Item", Quantity = 1, UnitPriceCents = 500 } };

        var result = _logic.HandleCreateTransaction(state, "cust-123", items);

        Assert.NotNull(result);
        Assert.Single(result.Pages);
        Assert.EndsWith("TransactionCreated", result.Pages[0].Event.TypeUrl);
    }

    [Fact]
    public void HandleCreateTransaction_AlreadyExists_Throws()
    {
        var state = new TransactionState("cust-1", new[] { new LineItem() }, 1000, 0, "", TransactionStatus.Pending);
        var items = new[] { new LineItem { Name = "Item", Quantity = 1, UnitPriceCents = 500 } };

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleCreateTransaction(state, "cust-123", items));

        Assert.Equal(StatusCode.FailedPrecondition, ex.StatusCode);
    }

    [Fact]
    public void HandleCreateTransaction_EmptyCustomerId_Throws()
    {
        var state = TransactionState.Empty;
        var items = new[] { new LineItem { Name = "Item", Quantity = 1, UnitPriceCents = 500 } };

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleCreateTransaction(state, "", items));

        Assert.Equal(StatusCode.InvalidArgument, ex.StatusCode);
    }

    [Fact]
    public void HandleCreateTransaction_NoItems_Throws()
    {
        var state = TransactionState.Empty;

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleCreateTransaction(state, "cust-123", Array.Empty<LineItem>()));

        Assert.Equal(StatusCode.InvalidArgument, ex.StatusCode);
    }

    [Fact]
    public void HandleApplyDiscount_PercentageSuccess_ReturnsEventBook()
    {
        var state = new TransactionState("cust-1", Array.Empty<LineItem>(), 1000, 0, "", TransactionStatus.Pending);

        var result = _logic.HandleApplyDiscount(state, "percentage", 10, null);

        Assert.NotNull(result);
        Assert.Single(result.Pages);
        Assert.EndsWith("DiscountApplied", result.Pages[0].Event.TypeUrl);
    }

    [Fact]
    public void HandleApplyDiscount_NotPending_Throws()
    {
        var state = new TransactionState("cust-1", Array.Empty<LineItem>(), 1000, 0, "", TransactionStatus.Completed);

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleApplyDiscount(state, "percentage", 10, null));

        Assert.Equal(StatusCode.FailedPrecondition, ex.StatusCode);
    }

    [Fact]
    public void HandleApplyDiscount_InvalidPercentage_Throws()
    {
        var state = new TransactionState("cust-1", Array.Empty<LineItem>(), 1000, 0, "", TransactionStatus.Pending);

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleApplyDiscount(state, "percentage", 150, null));

        Assert.Equal(StatusCode.InvalidArgument, ex.StatusCode);
    }

    [Fact]
    public void HandleCompleteTransaction_Success_ReturnsEventBook()
    {
        var state = new TransactionState("cust-1", Array.Empty<LineItem>(), 1000, 100, "percentage", TransactionStatus.Pending);

        var result = _logic.HandleCompleteTransaction(state, "card");

        Assert.NotNull(result);
        Assert.Single(result.Pages);
        Assert.EndsWith("TransactionCompleted", result.Pages[0].Event.TypeUrl);
    }

    [Fact]
    public void HandleCompleteTransaction_NotPending_Throws()
    {
        var state = new TransactionState("cust-1", Array.Empty<LineItem>(), 1000, 0, "", TransactionStatus.New);

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleCompleteTransaction(state, "card"));

        Assert.Equal(StatusCode.FailedPrecondition, ex.StatusCode);
    }

    [Fact]
    public void HandleCancelTransaction_Success_ReturnsEventBook()
    {
        var state = new TransactionState("cust-1", Array.Empty<LineItem>(), 1000, 0, "", TransactionStatus.Pending);

        var result = _logic.HandleCancelTransaction(state, "customer request");

        Assert.NotNull(result);
        Assert.Single(result.Pages);
        Assert.EndsWith("TransactionCancelled", result.Pages[0].Event.TypeUrl);
    }

    [Fact]
    public void HandleCancelTransaction_NotPending_Throws()
    {
        var state = new TransactionState("cust-1", Array.Empty<LineItem>(), 1000, 0, "", TransactionStatus.Completed);

        var ex = Assert.Throws<CommandValidationException>(
            () => _logic.HandleCancelTransaction(state, "customer request"));

        Assert.Equal(StatusCode.FailedPrecondition, ex.StatusCode);
    }
}

public class TransactionStateTests
{
    [Fact]
    public void Empty_ReturnsNewState()
    {
        var state = TransactionState.Empty;

        Assert.True(state.IsNew);
        Assert.False(state.IsPending);
    }

    [Fact]
    public void CalculateFinalTotal_SubtractsDiscount()
    {
        var state = new TransactionState("cust-1", Array.Empty<LineItem>(), 1000, 200, "fixed", TransactionStatus.Pending);

        Assert.Equal(800, state.CalculateFinalTotal());
    }

    [Fact]
    public void CalculateFinalTotal_NeverNegative()
    {
        var state = new TransactionState("cust-1", Array.Empty<LineItem>(), 100, 500, "fixed", TransactionStatus.Pending);

        Assert.Equal(0, state.CalculateFinalTotal());
    }

    [Fact]
    public void CalculateLoyaltyPoints_OneDollarOnePoint()
    {
        var state = new TransactionState("cust-1", Array.Empty<LineItem>(), 1500, 0, "", TransactionStatus.Pending);

        Assert.Equal(15, state.CalculateLoyaltyPoints());
    }
}
