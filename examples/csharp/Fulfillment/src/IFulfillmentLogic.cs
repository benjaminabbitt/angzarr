using Angzarr;

namespace Angzarr.Examples.Fulfillment;

public interface IFulfillmentLogic
{
    FulfillmentState RebuildState(EventBook? eventBook);
    EventBook HandleCreateShipment(FulfillmentState state, string orderId);
    EventBook HandleMarkPicked(FulfillmentState state);
    EventBook HandleMarkPacked(FulfillmentState state);
    EventBook HandleShip(FulfillmentState state, string trackingNumber, string carrier);
    EventBook HandleRecordDelivery(FulfillmentState state);
}
