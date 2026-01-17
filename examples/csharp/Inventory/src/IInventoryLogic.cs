using Angzarr;

namespace Angzarr.Examples.Inventory;

public interface IInventoryLogic
{
    InventoryState RebuildState(EventBook? eventBook);
    EventBook HandleInitializeStock(InventoryState state, string productId, int quantity);
    EventBook HandleReceiveStock(InventoryState state, int quantity);
    EventBook HandleReserveStock(InventoryState state, string orderId, int quantity);
    EventBook HandleReleaseReservation(InventoryState state, string orderId);
    EventBook HandleCommitReservation(InventoryState state, string orderId);
}
