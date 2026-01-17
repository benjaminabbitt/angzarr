using Angzarr;
using Examples;
using Grpc.Core;
using Serilog;

namespace Angzarr.Examples.Inventory;

public class InventoryService : BusinessLogic.BusinessLogicBase
{
    private readonly IInventoryLogic _logic;
    private readonly Serilog.ILogger _logger;

    public InventoryService(IInventoryLogic logic, Serilog.ILogger logger)
    {
        _logic = logic;
        _logger = logger.ForContext<InventoryService>();
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

        if (typeUrl.EndsWith("InitializeStock"))
        {
            var cmd = command.Unpack<InitializeStock>();
            result = _logic.HandleInitializeStock(state, cmd.ProductId, cmd.Quantity);
        }
        else if (typeUrl.EndsWith("ReceiveStock"))
        {
            var cmd = command.Unpack<ReceiveStock>();
            result = _logic.HandleReceiveStock(state, cmd.Quantity);
        }
        else if (typeUrl.EndsWith("ReserveStock"))
        {
            var cmd = command.Unpack<ReserveStock>();
            result = _logic.HandleReserveStock(state, cmd.OrderId, cmd.Quantity);
        }
        else if (typeUrl.EndsWith("ReleaseReservation"))
        {
            var cmd = command.Unpack<ReleaseReservation>();
            result = _logic.HandleReleaseReservation(state, cmd.OrderId);
        }
        else if (typeUrl.EndsWith("CommitReservation"))
        {
            var cmd = command.Unpack<CommitReservation>();
            result = _logic.HandleCommitReservation(state, cmd.OrderId);
        }
        else
        {
            throw CommandValidationException.InvalidArgument($"Unknown command type: {typeUrl}");
        }

        result.Cover = cmdBook.Cover;
        return result;
    }
}
