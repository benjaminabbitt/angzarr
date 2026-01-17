using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Serilog;

namespace Angzarr.Examples.Order;

public class OrderLogic : IOrderLogic
{
    private readonly Serilog.ILogger _logger;

    public OrderLogic(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<OrderLogic>();
    }

    public OrderState RebuildState(EventBook? eventBook)
    {
        var state = OrderState.Empty;

        if (eventBook == null || eventBook.Pages.Count == 0)
            return state;

        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;
            state = ApplyEvent(state, page.Event);
        }

        return state;
    }

    private OrderState ApplyEvent(OrderState state, Any eventAny)
    {
        var typeUrl = eventAny.TypeUrl;

        if (typeUrl.EndsWith("OrderCreated"))
        {
            var evt = eventAny.Unpack<OrderCreated>();
            return new OrderState(evt.CustomerId, evt.Items.ToList(), evt.SubtotalCents, evt.DiscountCents, 0, 0, "", "pending_payment");
        }

        if (typeUrl.EndsWith("LoyaltyDiscountApplied"))
        {
            var evt = eventAny.Unpack<LoyaltyDiscountApplied>();
            return state with { LoyaltyPointsUsed = evt.PointsUsed, DiscountCents = state.DiscountCents + evt.DiscountCents };
        }

        if (typeUrl.EndsWith("PaymentSubmitted"))
        {
            var evt = eventAny.Unpack<PaymentSubmitted>();
            return state with { PaymentMethod = evt.PaymentMethod, FinalTotalCents = evt.AmountCents, Status = "paid" };
        }

        if (typeUrl.EndsWith("OrderCompleted"))
        {
            return state with { Status = "completed" };
        }

        if (typeUrl.EndsWith("OrderCancelled"))
        {
            return state with { Status = "cancelled" };
        }

        return state;
    }

    public EventBook HandleCreateOrder(OrderState state, string customerId, IEnumerable<LineItem> items, int subtotalCents, int discountCents)
    {
        if (state.Exists)
            throw CommandValidationException.FailedPrecondition("Order already exists");

        if (string.IsNullOrWhiteSpace(customerId))
            throw CommandValidationException.InvalidArgument("Customer ID is required");

        var itemList = items.ToList();
        if (!itemList.Any())
            throw CommandValidationException.InvalidArgument("Order must have at least one item");

        _logger.Information("creating_order {@Data}", new { customer_id = customerId, subtotal = subtotalCents, discount = discountCents });

        var evt = new OrderCreated
        {
            CustomerId = customerId,
            SubtotalCents = subtotalCents,
            DiscountCents = discountCents,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.Items.AddRange(itemList);

        return CreateEventBook(evt);
    }

    public EventBook HandleApplyLoyaltyDiscount(OrderState state, int pointsUsed, int discountCents)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Order does not exist");

        if (!state.IsPendingPayment)
            throw CommandValidationException.FailedPrecondition("Order is not pending payment");

        if (pointsUsed <= 0)
            throw CommandValidationException.InvalidArgument("Points must be positive");

        _logger.Information("applying_loyalty_discount {@Data}", new { points = pointsUsed, discount_cents = discountCents });

        var evt = new LoyaltyDiscountApplied
        {
            PointsUsed = pointsUsed,
            DiscountCents = discountCents
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleSubmitPayment(OrderState state, string paymentMethod, int amountCents)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Order does not exist");

        if (!state.IsPendingPayment)
            throw CommandValidationException.FailedPrecondition("Order is not pending payment");

        if (string.IsNullOrWhiteSpace(paymentMethod))
            throw CommandValidationException.InvalidArgument("Payment method is required");

        var expectedAmount = state.SubtotalCents - state.DiscountCents;
        if (amountCents != expectedAmount)
            throw CommandValidationException.InvalidArgument($"Payment amount {amountCents} does not match expected {expectedAmount}");

        _logger.Information("submitting_payment {@Data}", new { method = paymentMethod, amount = amountCents });

        var evt = new PaymentSubmitted
        {
            PaymentMethod = paymentMethod,
            AmountCents = amountCents,
            SubmittedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleConfirmPayment(OrderState state)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Order does not exist");

        if (!state.IsPaid)
            throw CommandValidationException.FailedPrecondition("Order payment not submitted");

        _logger.Information("confirming_payment {@Data}", new { customer_id = state.CustomerId, total = state.FinalTotalCents });

        var evt = new OrderCompleted
        {
            CompletedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleCancelOrder(OrderState state, string reason)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Order does not exist");

        if (state.IsCancelled)
            throw CommandValidationException.FailedPrecondition("Order already cancelled");

        if (state.IsCompleted)
            throw CommandValidationException.FailedPrecondition("Cannot cancel completed order");

        _logger.Information("cancelling_order {@Data}", new { customer_id = state.CustomerId, reason });

        var evt = new OrderCancelled
        {
            Reason = reason ?? "",
            LoyaltyPointsUsed = state.LoyaltyPointsUsed,
            CancelledAt = Timestamp.FromDateTime(DateTime.UtcNow)
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
