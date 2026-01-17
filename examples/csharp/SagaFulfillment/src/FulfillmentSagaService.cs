using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Serilog;

namespace Angzarr.Examples.SagaFulfillment;

public class FulfillmentSagaService : Saga.SagaBase
{
    private readonly Serilog.ILogger _logger;

    public FulfillmentSagaService(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<FulfillmentSagaService>();
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
            if (!typeUrl.EndsWith("OrderCompleted")) continue;

            var orderId = eventBook.Cover?.Root?.Value != null
                ? Convert.ToHexString(eventBook.Cover.Root.Value.ToByteArray()).ToLower()
                : "";

            if (string.IsNullOrEmpty(orderId)) continue;

            _logger.Information("triggering_fulfillment {@Data}", new { order_id = orderId });

            var createShipmentCmd = new CreateShipment { OrderId = orderId };

            var cmdBook = new CommandBook
            {
                Cover = new Cover
                {
                    Domain = "fulfillment",
                    Root = eventBook.Cover?.Root
                },
                CorrelationId = eventBook.CorrelationId
            };
            cmdBook.Pages.Add(new CommandPage
            {
                Sequence = 0,
                Synchronous = false,
                Command = Any.Pack(createShipmentCmd)
            });

            commands.Add(cmdBook);
        }

        if (commands.Count > 0)
        {
            _logger.Information("processed_order_completed {@Data}", new { command_count = commands.Count });
        }

        return commands;
    }
}
