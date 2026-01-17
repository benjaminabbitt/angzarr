using Angzarr;
using Examples;
using Grpc.Core;
using Serilog;

namespace Angzarr.Examples.Product;

public class ProductService : BusinessLogic.BusinessLogicBase
{
    private readonly IProductLogic _logic;
    private readonly Serilog.ILogger _logger;

    public ProductService(IProductLogic logic, Serilog.ILogger logger)
    {
        _logic = logic;
        _logger = logger.ForContext<ProductService>();
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

        if (typeUrl.EndsWith("CreateProduct"))
        {
            var cmd = command.Unpack<CreateProduct>();
            result = _logic.HandleCreateProduct(state, cmd.Sku, cmd.Name, cmd.Description, cmd.PriceCents);
        }
        else if (typeUrl.EndsWith("UpdateProduct"))
        {
            var cmd = command.Unpack<UpdateProduct>();
            result = _logic.HandleUpdateProduct(state, cmd.Name, cmd.Description);
        }
        else if (typeUrl.EndsWith("SetPrice"))
        {
            var cmd = command.Unpack<SetPrice>();
            result = _logic.HandleSetPrice(state, cmd.PriceCents);
        }
        else if (typeUrl.EndsWith("Discontinue"))
        {
            result = _logic.HandleDiscontinue(state);
        }
        else
        {
            throw CommandValidationException.InvalidArgument($"Unknown command type: {typeUrl}");
        }

        result.Cover = cmdBook.Cover;
        return result;
    }
}
