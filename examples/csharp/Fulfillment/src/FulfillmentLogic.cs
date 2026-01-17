using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Serilog;

namespace Angzarr.Examples.Fulfillment;

public class FulfillmentLogic : IFulfillmentLogic
{
    private readonly Serilog.ILogger _logger;

    public FulfillmentLogic(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<FulfillmentLogic>();
    }

    public FulfillmentState RebuildState(EventBook? eventBook)
    {
        var state = FulfillmentState.Empty;

        if (eventBook == null || eventBook.Pages.Count == 0)
            return state;

        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;
            state = ApplyEvent(state, page.Event);
        }

        return state;
    }

    private FulfillmentState ApplyEvent(FulfillmentState state, Any eventAny)
    {
        var typeUrl = eventAny.TypeUrl;

        if (typeUrl.EndsWith("ShipmentCreated"))
        {
            var evt = eventAny.Unpack<ShipmentCreated>();
            return state with { OrderId = evt.OrderId, Status = "pending" };
        }

        if (typeUrl.EndsWith("ItemsPicked"))
        {
            return state with { Status = "picking" };
        }

        if (typeUrl.EndsWith("ItemsPacked"))
        {
            return state with { Status = "packing" };
        }

        if (typeUrl.EndsWith("Shipped"))
        {
            var evt = eventAny.Unpack<Shipped>();
            return state with { Status = "shipped", TrackingNumber = evt.TrackingNumber };
        }

        if (typeUrl.EndsWith("Delivered"))
        {
            return state with { Status = "delivered" };
        }

        return state;
    }

    public EventBook HandleCreateShipment(FulfillmentState state, string orderId)
    {
        if (state.Exists)
            throw CommandValidationException.FailedPrecondition("Shipment already exists");

        if (string.IsNullOrWhiteSpace(orderId))
            throw CommandValidationException.InvalidArgument("Order ID is required");

        _logger.Information("creating_shipment {@Data}", new { order_id = orderId });

        var evt = new ShipmentCreated
        {
            OrderId = orderId,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleMarkPicked(FulfillmentState state)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Shipment does not exist");

        if (!state.IsPending)
            throw CommandValidationException.FailedPrecondition("Shipment not in pending state");

        _logger.Information("marking_picked {@Data}", new { order_id = state.OrderId });

        var evt = new ItemsPicked
        {
            PickedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleMarkPacked(FulfillmentState state)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Shipment does not exist");

        if (!state.IsPicking)
            throw CommandValidationException.FailedPrecondition("Items not picked yet");

        _logger.Information("marking_packed {@Data}", new { order_id = state.OrderId });

        var evt = new ItemsPacked
        {
            PackedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleShip(FulfillmentState state, string trackingNumber, string carrier)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Shipment does not exist");

        if (!state.IsPacking)
            throw CommandValidationException.FailedPrecondition("Items not packed yet");

        if (string.IsNullOrWhiteSpace(trackingNumber))
            throw CommandValidationException.InvalidArgument("Tracking number is required");

        _logger.Information("shipping {@Data}", new { order_id = state.OrderId, tracking = trackingNumber, carrier });

        var evt = new Shipped
        {
            TrackingNumber = trackingNumber,
            Carrier = carrier ?? "",
            ShippedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleRecordDelivery(FulfillmentState state)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Shipment does not exist");

        if (!state.IsShipped)
            throw CommandValidationException.FailedPrecondition("Order not shipped yet");

        _logger.Information("recording_delivery {@Data}", new { order_id = state.OrderId });

        var evt = new Delivered
        {
            DeliveredAt = Timestamp.FromDateTime(DateTime.UtcNow)
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
