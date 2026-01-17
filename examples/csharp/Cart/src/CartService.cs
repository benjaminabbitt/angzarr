using Angzarr;
using Examples;
using Grpc.Core;
using Serilog;

namespace Angzarr.Examples.Cart;

public class CartService : BusinessLogic.BusinessLogicBase
{
    private readonly ICartLogic _logic;
    private readonly Serilog.ILogger _logger;

    public CartService(ICartLogic logic, Serilog.ILogger logger)
    {
        _logic = logic;
        _logger = logger.ForContext<CartService>();
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

        if (typeUrl.EndsWith("CreateCart"))
        {
            var cmd = command.Unpack<CreateCart>();
            result = _logic.HandleCreateCart(state, cmd.CustomerId);
        }
        else if (typeUrl.EndsWith("AddItem"))
        {
            var cmd = command.Unpack<AddItem>();
            result = _logic.HandleAddItem(state, cmd.ProductId, cmd.Name, cmd.Quantity, cmd.UnitPriceCents);
        }
        else if (typeUrl.EndsWith("UpdateQuantity"))
        {
            var cmd = command.Unpack<UpdateQuantity>();
            result = _logic.HandleUpdateQuantity(state, cmd.ProductId, cmd.Quantity);
        }
        else if (typeUrl.EndsWith("RemoveItem"))
        {
            var cmd = command.Unpack<RemoveItem>();
            result = _logic.HandleRemoveItem(state, cmd.ProductId);
        }
        else if (typeUrl.EndsWith("ApplyCoupon"))
        {
            var cmd = command.Unpack<ApplyCoupon>();
            result = _logic.HandleApplyCoupon(state, cmd.CouponCode, cmd.DiscountCents);
        }
        else if (typeUrl.EndsWith("ClearCart"))
        {
            result = _logic.HandleClearCart(state);
        }
        else if (typeUrl.EndsWith("Checkout"))
        {
            var cmd = command.Unpack<Checkout>();
            result = _logic.HandleCheckout(state, cmd.LoyaltyPointsToUse);
        }
        else
        {
            throw CommandValidationException.InvalidArgument($"Unknown command type: {typeUrl}");
        }

        result.Cover = cmdBook.Cover;
        return result;
    }
}
