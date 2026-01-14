using Angzarr;
using Examples;
using Grpc.Core;
using Serilog;

namespace Angzarr.Examples.Customer;

/// <summary>
/// gRPC service implementation for customer business logic.
/// </summary>
public class CustomerService : BusinessLogic.BusinessLogicBase
{
    private readonly ICustomerLogic _logic;
    private readonly Serilog.ILogger _logger;

    public CustomerService(ICustomerLogic logic, Serilog.ILogger logger)
    {
        _logic = logic;
        _logger = logger.ForContext<CustomerService>();
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

        if (typeUrl.EndsWith("CreateCustomer"))
        {
            var cmd = command.Unpack<CreateCustomer>();
            result = _logic.HandleCreateCustomer(state, cmd.Name, cmd.Email);
        }
        else if (typeUrl.EndsWith("AddLoyaltyPoints"))
        {
            var cmd = command.Unpack<AddLoyaltyPoints>();
            result = _logic.HandleAddLoyaltyPoints(state, cmd.Points, cmd.Reason);
        }
        else if (typeUrl.EndsWith("RedeemLoyaltyPoints"))
        {
            var cmd = command.Unpack<RedeemLoyaltyPoints>();
            result = _logic.HandleRedeemLoyaltyPoints(state, cmd.Points, cmd.RedemptionType);
        }
        else
        {
            throw CommandValidationException.InvalidArgument($"Unknown command type: {typeUrl}");
        }

        // Add cover from command book
        result.Cover = cmdBook.Cover;
        return result;
    }
}
