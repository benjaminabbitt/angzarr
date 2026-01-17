using Angzarr;
using Examples;
using Grpc.Core;
using Serilog;

namespace Angzarr.Examples.Fulfillment;

public class FulfillmentService : BusinessLogic.BusinessLogicBase
{
    private readonly IFulfillmentLogic _logic;
    private readonly Serilog.ILogger _logger;

    public FulfillmentService(IFulfillmentLogic logic, Serilog.ILogger logger)
    {
        _logic = logic;
        _logger = logger.ForContext<FulfillmentService>();
    }

    public override Task<BusinessResponse> Handle(ContextualCommand request, ServerCallContext context)
    {
        try
        {
            var events = ProcessCommand(request);
            var response = new BusinessResponse { Events = events };
            return Task.FromResult(response);
        }
        catch (CommandValidationException ex)
        {
            throw new RpcException(new Status(ex.StatusCode, ex.Message));
        }
        catch (Exception ex)
        {
            _logger.Error(ex, "Unexpected error processing command");
            throw new RpcException(new Status(StatusCode.Internal, $"Internal error: {ex.Message}"));
        }
    }

    private EventBook ProcessCommand(ContextualCommand request)
    {
        var cmdBook = request.Command;
        var priorEvents = request.Events;

        if (cmdBook == null || cmdBook.Pages.Count == 0)
            throw CommandValidationException.InvalidArgument("CommandBook has no pages");

        var cmdPage = cmdBook.Pages[0];
        if (cmdPage.Command == null)
            throw CommandValidationException.InvalidArgument("Command page has no command");

        var state = _logic.RebuildState(priorEvents);
        var command = cmdPage.Command;
        var typeUrl = command.TypeUrl;

        EventBook result;

        if (typeUrl.EndsWith("CreateShipment"))
        {
            var cmd = command.Unpack<CreateShipment>();
            result = _logic.HandleCreateShipment(state, cmd.OrderId);
        }
        else if (typeUrl.EndsWith("MarkPicked"))
        {
            result = _logic.HandleMarkPicked(state);
        }
        else if (typeUrl.EndsWith("MarkPacked"))
        {
            result = _logic.HandleMarkPacked(state);
        }
        else if (typeUrl.EndsWith("Ship"))
        {
            var cmd = command.Unpack<Ship>();
            result = _logic.HandleShip(state, cmd.TrackingNumber, cmd.Carrier);
        }
        else if (typeUrl.EndsWith("RecordDelivery"))
        {
            result = _logic.HandleRecordDelivery(state);
        }
        else
        {
            throw CommandValidationException.InvalidArgument($"Unknown command type: {typeUrl}");
        }

        result.Cover = cmdBook.Cover;
        return result;
    }
}
