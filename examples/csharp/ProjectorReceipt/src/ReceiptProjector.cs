using System.Text;
using Angzarr;
using Examples;
using Google.Protobuf.WellKnownTypes;
using Serilog;

namespace Angzarr.Examples.Projector;

public class ReceiptProjector : IReceiptProjector
{
    private readonly Serilog.ILogger _logger;
    private const string ProjectorName = "receipt";
    private const int PointsPerDollar = 10;

    public ReceiptProjector(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<ReceiptProjector>();
    }

    public Projection? Project(EventBook eventBook)
    {
        if (eventBook.Pages.Count == 0)
            return null;

        var state = RebuildState(eventBook);
        if (!state.Completed)
            return null;

        var orderId = eventBook.Cover?.Root != null
            ? Convert.ToHexString(eventBook.Cover.Root.Value.ToByteArray()).ToLower()
            : "";

        var shortId = orderId.Length > 16 ? orderId[..16] : orderId;
        var loyaltyPointsEarned = (state.FinalTotalCents / 100) * PointsPerDollar;

        _logger.Information("generated_receipt {@Data}",
            new { order_id = shortId, total_cents = state.FinalTotalCents, payment_method = state.PaymentMethod });

        var receipt = new Receipt
        {
            OrderId = orderId,
            CustomerId = state.CustomerId,
            SubtotalCents = state.SubtotalCents,
            DiscountCents = state.DiscountCents,
            FinalTotalCents = state.FinalTotalCents,
            PaymentMethod = state.PaymentMethod,
            LoyaltyPointsEarned = loyaltyPointsEarned,
            FormattedText = FormatReceipt(orderId, state, loyaltyPointsEarned)
        };
        receipt.Items.AddRange(state.Items);

        var sequence = eventBook.Pages.Count > 0 ? (uint)eventBook.Pages[^1].Num : 0;

        return new Projection
        {
            Cover = eventBook.Cover,
            Projector = ProjectorName,
            Sequence = sequence,
            Projection_ = Any.Pack(receipt)
        };
    }

    private record ProjectionState(
        string CustomerId,
        List<LineItem> Items,
        int SubtotalCents,
        int DiscountCents,
        int LoyaltyPointsUsed,
        int FinalTotalCents,
        string PaymentMethod,
        bool Completed);

    private ProjectionState RebuildState(EventBook eventBook)
    {
        var state = new ProjectionState("", new List<LineItem>(), 0, 0, 0, 0, "", false);

        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;
            var typeUrl = page.Event.TypeUrl;

            if (typeUrl.EndsWith("OrderCreated"))
            {
                var evt = page.Event.Unpack<OrderCreated>();
                state = state with
                {
                    CustomerId = evt.CustomerId,
                    Items = evt.Items.ToList(),
                    SubtotalCents = evt.SubtotalCents,
                    DiscountCents = evt.DiscountCents
                };
            }
            else if (typeUrl.EndsWith("LoyaltyDiscountApplied"))
            {
                var evt = page.Event.Unpack<LoyaltyDiscountApplied>();
                state = state with
                {
                    LoyaltyPointsUsed = evt.PointsUsed,
                    DiscountCents = state.DiscountCents + evt.DiscountCents
                };
            }
            else if (typeUrl.EndsWith("PaymentSubmitted"))
            {
                var evt = page.Event.Unpack<PaymentSubmitted>();
                state = state with
                {
                    PaymentMethod = evt.PaymentMethod,
                    FinalTotalCents = evt.AmountCents
                };
            }
            else if (typeUrl.EndsWith("OrderCompleted"))
            {
                state = state with { Completed = true };
            }
        }

        return state;
    }

    private static string FormatReceipt(string orderId, ProjectionState state, int loyaltyPointsEarned)
    {
        var sb = new StringBuilder();
        var line = new string('═', 40);
        var thinLine = new string('─', 40);

        var shortOrderId = orderId.Length > 16 ? orderId[..16] : orderId;
        var shortCustId = state.CustomerId.Length > 16 ? state.CustomerId[..16] : state.CustomerId;

        sb.AppendLine(line);
        sb.AppendLine("           RECEIPT");
        sb.AppendLine(line);
        sb.AppendLine($"Order: {shortOrderId}...");
        sb.AppendLine($"Customer: {(string.IsNullOrEmpty(shortCustId) ? "N/A" : shortCustId + "...")}");
        sb.AppendLine(thinLine);

        foreach (var item in state.Items)
        {
            var lineTotal = item.Quantity * item.UnitPriceCents;
            sb.AppendLine($"{item.Quantity} x {item.Name} @ ${item.UnitPriceCents / 100.0:F2} = ${lineTotal / 100.0:F2}");
        }

        sb.AppendLine(thinLine);
        sb.AppendLine($"Subtotal:              ${state.SubtotalCents / 100.0:F2}");

        if (state.DiscountCents > 0)
        {
            var discountType = state.LoyaltyPointsUsed > 0 ? "loyalty" : "coupon";
            sb.AppendLine($"Discount ({discountType}):       -${state.DiscountCents / 100.0:F2}");
        }

        sb.AppendLine(thinLine);
        sb.AppendLine($"TOTAL:                 ${state.FinalTotalCents / 100.0:F2}");
        sb.AppendLine($"Payment: {state.PaymentMethod}");
        sb.AppendLine(thinLine);
        sb.AppendLine($"Loyalty Points Earned: {loyaltyPointsEarned}");
        sb.AppendLine(line);
        sb.AppendLine("     Thank you for your purchase!");
        sb.Append(line);

        return sb.ToString();
    }
}
