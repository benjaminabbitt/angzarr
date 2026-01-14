using Angzarr;
using Examples;
using Google.Protobuf.WellKnownTypes;
using Serilog;

namespace Angzarr.Examples.Saga;

public class LoyaltySaga : ILoyaltySaga
{
    private readonly Serilog.ILogger _logger;

    public LoyaltySaga(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<LoyaltySaga>();
    }

    public IReadOnlyList<CommandBook> ProcessEvents(EventBook eventBook)
    {
        if (eventBook.Pages.Count == 0)
            return Array.Empty<CommandBook>();

        var commands = new List<CommandBook>();

        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;
            if (!page.Event.TypeUrl.EndsWith("TransactionCompleted")) continue;

            var completed = page.Event.Unpack<TransactionCompleted>();
            var points = completed.LoyaltyPointsEarned;

            if (points <= 0) continue;

            var customerId = eventBook.Cover?.Root;
            if (customerId == null)
            {
                _logger.Warning("Transaction has no root ID, skipping loyalty points");
                continue;
            }

            var transactionId = Convert.ToHexString(customerId.Value.ToByteArray()).ToLower();
            var shortId = transactionId.Length > 16 ? transactionId[..16] : transactionId;

            _logger.Information("awarding_loyalty_points {@Data}",
                new { points, transaction_id = shortId });

            var addPoints = new AddLoyaltyPoints
            {
                Points = points,
                Reason = $"transaction:{transactionId}"
            };

            var commandBook = new CommandBook
            {
                Cover = new Cover
                {
                    Domain = "customer",
                    Root = customerId
                }
            };
            commandBook.Pages.Add(new CommandPage
            {
                Sequence = 0,
                Synchronous = false,
                Command = Any.Pack(addPoints)
            });

            commands.Add(commandBook);
        }

        return commands;
    }
}
