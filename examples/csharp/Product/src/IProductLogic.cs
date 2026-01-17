using Angzarr;

namespace Angzarr.Examples.Product;

public interface IProductLogic
{
    ProductState RebuildState(EventBook? eventBook);
    EventBook HandleCreateProduct(ProductState state, string sku, string name, string description, int priceCents);
    EventBook HandleUpdateProduct(ProductState state, string name, string description);
    EventBook HandleSetPrice(ProductState state, int priceCents);
    EventBook HandleDiscontinue(ProductState state);
}
