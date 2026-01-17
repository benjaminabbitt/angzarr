using Angzarr;
using Examples;

namespace Angzarr.Examples.Order;

public interface IOrderLogic
{
    OrderState RebuildState(EventBook? eventBook);
    EventBook HandleCreateOrder(OrderState state, string customerId, IEnumerable<LineItem> items, int subtotalCents, int discountCents);
    EventBook HandleApplyLoyaltyDiscount(OrderState state, int pointsUsed, int discountCents);
    EventBook HandleSubmitPayment(OrderState state, string paymentMethod, int amountCents);
    EventBook HandleConfirmPayment(OrderState state);
    EventBook HandleCancelOrder(OrderState state, string reason);
}
