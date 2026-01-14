using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Serilog;

namespace Angzarr.Examples.Transaction;

public class TransactionLogic : ITransactionLogic
{
    private readonly Serilog.ILogger _logger;

    public TransactionLogic(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<TransactionLogic>();
    }

    public TransactionState RebuildState(EventBook? eventBook)
    {
        var state = TransactionState.Empty;

        if (eventBook == null || eventBook.Pages.Count == 0)
            return state;

        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;
            state = ApplyEvent(state, page.Event);
        }

        return state;
    }

    private TransactionState ApplyEvent(TransactionState state, Any eventAny)
    {
        var typeUrl = eventAny.TypeUrl;

        if (typeUrl.EndsWith("TransactionCreated"))
        {
            var evt = eventAny.Unpack<TransactionCreated>();
            return new TransactionState(
                evt.CustomerId,
                evt.Items.ToList(),
                evt.SubtotalCents,
                0,
                "",
                TransactionStatus.Pending);
        }

        if (typeUrl.EndsWith("DiscountApplied"))
        {
            var evt = eventAny.Unpack<DiscountApplied>();
            return state with
            {
                DiscountCents = evt.DiscountCents,
                DiscountType = evt.DiscountType
            };
        }

        if (typeUrl.EndsWith("TransactionCompleted"))
        {
            return state with { Status = TransactionStatus.Completed };
        }

        if (typeUrl.EndsWith("TransactionCancelled"))
        {
            return state with { Status = TransactionStatus.Cancelled };
        }

        return state;
    }

    public EventBook HandleCreateTransaction(TransactionState state, string customerId, IEnumerable<LineItem> items)
    {
        if (!state.IsNew)
            throw CommandValidationException.FailedPrecondition("Transaction already exists");

        if (string.IsNullOrWhiteSpace(customerId))
            throw CommandValidationException.InvalidArgument("customer_id is required");

        var itemList = items.ToList();
        if (itemList.Count == 0)
            throw CommandValidationException.InvalidArgument("at least one item is required");

        var subtotal = itemList.Sum(i => i.Quantity * i.UnitPriceCents);

        _logger.Information("creating_transaction {@Data}",
            new { customer_id = customerId, item_count = itemList.Count, subtotal_cents = subtotal });

        var evt = new TransactionCreated
        {
            CustomerId = customerId,
            SubtotalCents = subtotal,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.Items.AddRange(itemList);

        return CreateEventBook(evt);
    }

    public EventBook HandleApplyDiscount(TransactionState state, string discountType, int value, string? couponCode)
    {
        if (!state.IsPending)
            throw CommandValidationException.FailedPrecondition("Can only apply discount to pending transaction");

        var discountCents = discountType switch
        {
            "percentage" when value < 0 || value > 100 =>
                throw CommandValidationException.InvalidArgument("Percentage must be 0-100"),
            "percentage" => (state.SubtotalCents * value) / 100,
            "fixed" => Math.Min(value, state.SubtotalCents),
            "coupon" => 500,
            _ => throw CommandValidationException.InvalidArgument($"Unknown discount type: {discountType}")
        };

        _logger.Information("applying_discount {@Data}",
            new { discount_type = discountType, value, discount_cents = discountCents });

        var evt = new DiscountApplied
        {
            DiscountType = discountType,
            Value = value,
            DiscountCents = discountCents,
            CouponCode = couponCode ?? ""
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleCompleteTransaction(TransactionState state, string paymentMethod)
    {
        if (!state.IsPending)
            throw CommandValidationException.FailedPrecondition("Can only complete pending transaction");

        var finalTotal = state.CalculateFinalTotal();
        var loyaltyPoints = state.CalculateLoyaltyPoints();

        _logger.Information("completing_transaction {@Data}",
            new { final_total_cents = finalTotal, payment_method = paymentMethod, loyalty_points_earned = loyaltyPoints });

        var evt = new TransactionCompleted
        {
            FinalTotalCents = finalTotal,
            PaymentMethod = paymentMethod,
            LoyaltyPointsEarned = loyaltyPoints,
            CompletedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleCancelTransaction(TransactionState state, string reason)
    {
        if (!state.IsPending)
            throw CommandValidationException.FailedPrecondition("Can only cancel pending transaction");

        _logger.Information("cancelling_transaction {@Data}", new { reason });

        var evt = new TransactionCancelled
        {
            Reason = reason,
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
