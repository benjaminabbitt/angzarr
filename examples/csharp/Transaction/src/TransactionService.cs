using Angzarr;
using Examples;
using Grpc.Core;
using Serilog;

namespace Angzarr.Examples.Transaction;

public class TransactionService : BusinessLogic.BusinessLogicBase
{
    private readonly ITransactionLogic _logic;
    private readonly Serilog.ILogger _logger;

    public TransactionService(ITransactionLogic logic, Serilog.ILogger logger)
    {
        _logic = logic;
        _logger = logger.ForContext<TransactionService>();
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

        if (typeUrl.EndsWith("CreateTransaction"))
        {
            var cmd = command.Unpack<CreateTransaction>();
            result = _logic.HandleCreateTransaction(state, cmd.CustomerId, cmd.Items);
        }
        else if (typeUrl.EndsWith("ApplyDiscount"))
        {
            var cmd = command.Unpack<ApplyDiscount>();
            result = _logic.HandleApplyDiscount(state, cmd.DiscountType, cmd.Value, cmd.CouponCode);
        }
        else if (typeUrl.EndsWith("CompleteTransaction"))
        {
            var cmd = command.Unpack<CompleteTransaction>();
            result = _logic.HandleCompleteTransaction(state, cmd.PaymentMethod);
        }
        else if (typeUrl.EndsWith("CancelTransaction"))
        {
            var cmd = command.Unpack<CancelTransaction>();
            result = _logic.HandleCancelTransaction(state, cmd.Reason);
        }
        else
        {
            throw CommandValidationException.InvalidArgument($"Unknown command type: {typeUrl}");
        }

        result.Cover = cmdBook.Cover;
        return result;
    }
}
