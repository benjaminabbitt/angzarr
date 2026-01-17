using Angzarr;
using Examples;
using Grpc.Core;
using Serilog;

namespace Angzarr.Examples.Order;

public class OrderService : BusinessLogic.BusinessLogicBase
{
    private readonly IOrderLogic _logic;
    private readonly Serilog.ILogger _logger;

    public OrderService(IOrderLogic logic, Serilog.ILogger logger)
    {
        _logic = logic;
        _logger = logger.ForContext<OrderService>();
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

        if (typeUrl.EndsWith("CreateOrder"))
        {
            var cmd = command.Unpack<CreateOrder>();
            result = _logic.HandleCreateOrder(state, cmd.CustomerId, cmd.Items, cmd.SubtotalCents, cmd.DiscountCents);
        }
        else if (typeUrl.EndsWith("ApplyLoyaltyDiscount"))
        {
            var cmd = command.Unpack<ApplyLoyaltyDiscount>();
            result = _logic.HandleApplyLoyaltyDiscount(state, cmd.PointsUsed, cmd.DiscountCents);
        }
        else if (typeUrl.EndsWith("SubmitPayment"))
        {
            var cmd = command.Unpack<SubmitPayment>();
            result = _logic.HandleSubmitPayment(state, cmd.PaymentMethod, cmd.AmountCents);
        }
        else if (typeUrl.EndsWith("ConfirmPayment"))
        {
            result = _logic.HandleConfirmPayment(state);
        }
        else if (typeUrl.EndsWith("CancelOrder"))
        {
            var cmd = command.Unpack<CancelOrder>();
            result = _logic.HandleCancelOrder(state, cmd.Reason);
        }
        else
        {
            throw CommandValidationException.InvalidArgument($"Unknown command type: {typeUrl}");
        }

        result.Cover = cmdBook.Cover;
        return result;
    }
}
