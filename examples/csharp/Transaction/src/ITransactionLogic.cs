using Angzarr;
using Examples;

namespace Angzarr.Examples.Transaction;

public interface ITransactionLogic
{
    TransactionState RebuildState(EventBook? eventBook);
    EventBook HandleCreateTransaction(TransactionState state, string customerId, IEnumerable<LineItem> items);
    EventBook HandleApplyDiscount(TransactionState state, string discountType, int value, string? couponCode);
    EventBook HandleCompleteTransaction(TransactionState state, string paymentMethod);
    EventBook HandleCancelTransaction(TransactionState state, string reason);
}
