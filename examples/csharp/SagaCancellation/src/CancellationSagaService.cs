using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Serilog;

namespace Angzarr.Examples.SagaCancellation;

public class CancellationSagaService : Saga.SagaBase
{
    private readonly Serilog.ILogger _logger;

    public CancellationSagaService(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<CancellationSagaService>();
    }

    public override Task<Empty> Handle(EventBook request, ServerCallContext context)
    {
        ProcessEvents(request);
        return Task.FromResult(new Empty());
    }

    public override Task<SagaResponse> HandleSync(EventBook request, ServerCallContext context)
    {
        var commands = ProcessEvents(request);
        var response = new SagaResponse();
        response.Commands.AddRange(commands);
        return Task.FromResult(response);
    }

    private List<CommandBook> ProcessEvents(EventBook eventBook)
    {
        var commands = new List<CommandBook>();

        if (eventBook.Pages.Count == 0)
            return commands;

        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;

            var typeUrl = page.Event.TypeUrl;
            if (!typeUrl.EndsWith("OrderCancelled")) continue;

            var cancelledEvent = page.Event.Unpack<OrderCancelled>();

            var orderId = eventBook.Cover?.Root?.Value != null
                ? Convert.ToHexString(eventBook.Cover.Root.Value.ToByteArray()).ToLower()
                : "";

            if (string.IsNullOrEmpty(orderId)) continue;

            _logger.Information("processing_order_cancellation {@Data}", new { order_id = orderId });

            var releaseCmd = new ReleaseReservation { OrderId = orderId };

            var releaseCmdBook = new CommandBook
            {
                Cover = new Cover
                {
                    Domain = "inventory",
                    Root = eventBook.Cover?.Root
                },
                CorrelationId = eventBook.CorrelationId
            };
            releaseCmdBook.Pages.Add(new CommandPage
            {
                Sequence = 0,
                Synchronous = false,
                Command = Any.Pack(releaseCmd)
            });

            commands.Add(releaseCmdBook);

            if (cancelledEvent.LoyaltyPointsUsed > 0)
            {
                var addPointsCmd = new AddLoyaltyPoints
                {
                    Points = cancelledEvent.LoyaltyPointsUsed,
                    Reason = "Order cancellation refund"
                };

                var addPointsCmdBook = new CommandBook
                {
                    Cover = new Cover { Domain = "customer" },
                    CorrelationId = eventBook.CorrelationId
                };
                addPointsCmdBook.Pages.Add(new CommandPage
                {
                    Sequence = 0,
                    Synchronous = false,
                    Command = Any.Pack(addPointsCmd)
                });

                commands.Add(addPointsCmdBook);
            }
        }

        if (commands.Count > 0)
        {
            _logger.Information("processed_cancellation {@Data}", new { compensation_commands = commands.Count });
        }

        return commands;
    }
}
