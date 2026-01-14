using Angzarr;
using Examples;
using Serilog;

namespace Angzarr.Examples.Projector;

public interface ITransactionLogProjector
{
    void LogEvents(EventBook eventBook);
}

public class TransactionLogProjector : ITransactionLogProjector
{
    private readonly Serilog.ILogger _logger;

    public TransactionLogProjector(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<TransactionLogProjector>();
    }

    public void LogEvents(EventBook eventBook)
    {
        if (eventBook.Pages.Count == 0) return;

        var domain = eventBook.Cover?.Domain ?? "transaction";
        var rootId = eventBook.Cover?.Root != null
            ? Convert.ToHexString(eventBook.Cover.Root.Value.ToByteArray()).ToLower()
            : "";
        var shortId = rootId.Length > 16 ? rootId[..16] : rootId;

        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;

            var sequence = page.Num;
            var eventType = ExtractEventType(page.Event.TypeUrl);

            LogEventDetails(domain, shortId, sequence, eventType, page.Event);
        }
    }

    private static string ExtractEventType(string typeUrl)
    {
        var idx = typeUrl.LastIndexOf('.');
        return idx >= 0 ? typeUrl[(idx + 1)..] : typeUrl;
    }

    private void LogEventDetails(string domain, string rootId, uint sequence, string eventType, Google.Protobuf.WellKnownTypes.Any eventAny)
    {
        switch (eventType)
        {
            case "TransactionCreated":
                var created = eventAny.Unpack<TransactionCreated>();
                _logger.Information("event {@Data}", new
                {
                    domain, root_id = rootId, sequence, event_type = eventType,
                    customer_id = created.CustomerId, item_count = created.Items.Count,
                    subtotal_cents = created.SubtotalCents,
                    created_at = created.CreatedAt?.ToDateTime().ToString("O") ?? ""
                });
                break;

            case "DiscountApplied":
                var applied = eventAny.Unpack<DiscountApplied>();
                _logger.Information("event {@Data}", new
                {
                    domain, root_id = rootId, sequence, event_type = eventType,
                    discount_type = applied.DiscountType, value = applied.Value,
                    discount_cents = applied.DiscountCents
                });
                break;

            case "TransactionCompleted":
                var completed = eventAny.Unpack<TransactionCompleted>();
                _logger.Information("event {@Data}", new
                {
                    domain, root_id = rootId, sequence, event_type = eventType,
                    final_total_cents = completed.FinalTotalCents, payment_method = completed.PaymentMethod,
                    loyalty_points_earned = completed.LoyaltyPointsEarned,
                    completed_at = completed.CompletedAt?.ToDateTime().ToString("O") ?? ""
                });
                break;

            case "TransactionCancelled":
                var cancelled = eventAny.Unpack<TransactionCancelled>();
                _logger.Information("event {@Data}", new
                {
                    domain, root_id = rootId, sequence, event_type = eventType,
                    reason = cancelled.Reason,
                    cancelled_at = cancelled.CancelledAt?.ToDateTime().ToString("O") ?? ""
                });
                break;

            default:
                _logger.Information("event {@Data}", new
                {
                    domain, root_id = rootId, sequence, event_type = eventType,
                    raw_bytes = eventAny.Value.Length
                });
                break;
        }
    }
}
