using Angzarr;
using Examples;
using Serilog;

namespace Angzarr.Examples.Projector;

public interface ICustomerLogProjector
{
    void LogEvents(EventBook eventBook);
}

public class CustomerLogProjector : ICustomerLogProjector
{
    private readonly Serilog.ILogger _logger;

    public CustomerLogProjector(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<CustomerLogProjector>();
    }

    public void LogEvents(EventBook eventBook)
    {
        if (eventBook.Pages.Count == 0) return;

        var domain = eventBook.Cover?.Domain ?? "customer";
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
            case "CustomerCreated":
                var created = eventAny.Unpack<CustomerCreated>();
                _logger.Information("event {@Data}", new
                {
                    domain, root_id = rootId, sequence, event_type = eventType,
                    name = created.Name, email = created.Email,
                    created_at = created.CreatedAt?.ToDateTime().ToString("O") ?? ""
                });
                break;

            case "LoyaltyPointsAdded":
                var added = eventAny.Unpack<LoyaltyPointsAdded>();
                _logger.Information("event {@Data}", new
                {
                    domain, root_id = rootId, sequence, event_type = eventType,
                    points = added.Points, new_balance = added.NewBalance, reason = added.Reason
                });
                break;

            case "LoyaltyPointsRedeemed":
                var redeemed = eventAny.Unpack<LoyaltyPointsRedeemed>();
                _logger.Information("event {@Data}", new
                {
                    domain, root_id = rootId, sequence, event_type = eventType,
                    points = redeemed.Points, new_balance = redeemed.NewBalance, redemption_type = redeemed.RedemptionType
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
