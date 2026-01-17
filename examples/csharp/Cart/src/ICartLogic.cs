using Angzarr;

namespace Angzarr.Examples.Cart;

public interface ICartLogic
{
    CartState RebuildState(EventBook? eventBook);
    EventBook HandleCreateCart(CartState state, string customerId);
    EventBook HandleAddItem(CartState state, string productId, string name, int quantity, int unitPriceCents);
    EventBook HandleUpdateQuantity(CartState state, string productId, int quantity);
    EventBook HandleRemoveItem(CartState state, string productId);
    EventBook HandleApplyCoupon(CartState state, string couponCode, int discountCents);
    EventBook HandleClearCart(CartState state);
    EventBook HandleCheckout(CartState state, int loyaltyPointsToUse);
}
