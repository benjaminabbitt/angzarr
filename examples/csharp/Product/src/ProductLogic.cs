using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Serilog;

namespace Angzarr.Examples.Product;

public class ProductLogic : IProductLogic
{
    private readonly Serilog.ILogger _logger;

    public ProductLogic(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<ProductLogic>();
    }

    public ProductState RebuildState(EventBook? eventBook)
    {
        var state = ProductState.Empty;

        if (eventBook == null || eventBook.Pages.Count == 0)
            return state;

        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;
            state = ApplyEvent(state, page.Event);
        }

        return state;
    }

    private ProductState ApplyEvent(ProductState state, Any eventAny)
    {
        var typeUrl = eventAny.TypeUrl;

        if (typeUrl.EndsWith("ProductCreated"))
        {
            var evt = eventAny.Unpack<ProductCreated>();
            return new ProductState(evt.Sku, evt.Name, evt.Description, evt.PriceCents, "active");
        }

        if (typeUrl.EndsWith("ProductUpdated"))
        {
            var evt = eventAny.Unpack<ProductUpdated>();
            return state with { Name = evt.Name, Description = evt.Description };
        }

        if (typeUrl.EndsWith("PriceSet"))
        {
            var evt = eventAny.Unpack<PriceSet>();
            return state with { PriceCents = evt.NewPriceCents };
        }

        if (typeUrl.EndsWith("ProductDiscontinued"))
        {
            return state with { Status = "discontinued" };
        }

        return state;
    }

    public EventBook HandleCreateProduct(ProductState state, string sku, string name, string description, int priceCents)
    {
        if (state.Exists)
            throw CommandValidationException.FailedPrecondition("Product already exists");

        if (string.IsNullOrWhiteSpace(sku))
            throw CommandValidationException.InvalidArgument("SKU is required");

        if (string.IsNullOrWhiteSpace(name))
            throw CommandValidationException.InvalidArgument("Product name is required");

        if (priceCents <= 0)
            throw CommandValidationException.InvalidArgument("Price must be positive");

        _logger.Information("creating_product {@Data}", new { sku, name, price_cents = priceCents });

        var evt = new ProductCreated
        {
            Sku = sku,
            Name = name,
            Description = description ?? "",
            PriceCents = priceCents,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleUpdateProduct(ProductState state, string name, string description)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Product does not exist");

        if (state.IsDiscontinued)
            throw CommandValidationException.FailedPrecondition("Cannot update discontinued product");

        if (string.IsNullOrWhiteSpace(name))
            throw CommandValidationException.InvalidArgument("Product name is required");

        _logger.Information("updating_product {@Data}", new { sku = state.Sku, name });

        var evt = new ProductUpdated
        {
            Name = name,
            Description = description ?? ""
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleSetPrice(ProductState state, int priceCents)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Product does not exist");

        if (state.IsDiscontinued)
            throw CommandValidationException.FailedPrecondition("Cannot set price on discontinued product");

        if (priceCents <= 0)
            throw CommandValidationException.InvalidArgument("Price must be positive");

        _logger.Information("setting_price {@Data}", new { sku = state.Sku, old_price = state.PriceCents, new_price = priceCents });

        var evt = new PriceSet
        {
            OldPriceCents = state.PriceCents,
            NewPriceCents = priceCents
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleDiscontinue(ProductState state)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Product does not exist");

        if (state.IsDiscontinued)
            throw CommandValidationException.FailedPrecondition("Product already discontinued");

        _logger.Information("discontinuing_product {@Data}", new { sku = state.Sku });

        var evt = new ProductDiscontinued
        {
            DiscontinuedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    private static EventBook CreateEventBook(IMessage evt)
    {
        var page = new EventPage
        {
            Num = 0,
            Event = Any.Pack(evt),
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        var book = new EventBook();
        book.Pages.Add(page);
        return book;
    }
}
