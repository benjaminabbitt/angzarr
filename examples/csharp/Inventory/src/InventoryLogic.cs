using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Serilog;

namespace Angzarr.Examples.Inventory;

public class InventoryLogic : IInventoryLogic
{
    private const int LowStockThreshold = 10;
    private readonly Serilog.ILogger _logger;

    public InventoryLogic(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<InventoryLogic>();
    }

    public InventoryState RebuildState(EventBook? eventBook)
    {
        var state = InventoryState.Empty;

        if (eventBook == null || eventBook.Pages.Count == 0)
            return state;

        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;
            state = ApplyEvent(state, page.Event);
        }

        return state;
    }

    private InventoryState ApplyEvent(InventoryState state, Any eventAny)
    {
        var typeUrl = eventAny.TypeUrl;

        if (typeUrl.EndsWith("StockInitialized"))
        {
            var evt = eventAny.Unpack<StockInitialized>();
            return state with { ProductId = evt.ProductId, OnHand = evt.Quantity };
        }

        if (typeUrl.EndsWith("StockReceived"))
        {
            var evt = eventAny.Unpack<StockReceived>();
            return state with { OnHand = evt.NewOnHand };
        }

        if (typeUrl.EndsWith("StockReserved"))
        {
            var evt = eventAny.Unpack<StockReserved>();
            var newReservations = new List<Reservation>(state.Reservations) { new(evt.OrderId, evt.Quantity) };
            return state with { Reserved = evt.NewReserved, Reservations = newReservations };
        }

        if (typeUrl.EndsWith("ReservationReleased"))
        {
            var evt = eventAny.Unpack<ReservationReleased>();
            var remainingReservations = state.Reservations.Where(r => r.OrderId != evt.OrderId).ToList();
            return state with { Reserved = evt.NewReserved, Reservations = remainingReservations };
        }

        if (typeUrl.EndsWith("ReservationCommitted"))
        {
            var evt = eventAny.Unpack<ReservationCommitted>();
            var remainingReservations = state.Reservations.Where(r => r.OrderId != evt.OrderId).ToList();
            return state with { OnHand = evt.NewOnHand, Reserved = evt.NewReserved, Reservations = remainingReservations };
        }

        return state;
    }

    public EventBook HandleInitializeStock(InventoryState state, string productId, int quantity)
    {
        if (state.Exists)
            throw CommandValidationException.FailedPrecondition("Inventory already initialized");

        if (string.IsNullOrWhiteSpace(productId))
            throw CommandValidationException.InvalidArgument("Product ID is required");

        if (quantity < 0)
            throw CommandValidationException.InvalidArgument("Quantity cannot be negative");

        _logger.Information("initializing_stock {@Data}", new { product_id = productId, quantity });

        var evt = new StockInitialized
        {
            ProductId = productId,
            Quantity = quantity
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleReceiveStock(InventoryState state, int quantity)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Inventory not initialized");

        if (quantity <= 0)
            throw CommandValidationException.InvalidArgument("Quantity must be positive");

        var newOnHand = state.OnHand + quantity;

        _logger.Information("receiving_stock {@Data}", new { product_id = state.ProductId, quantity, new_on_hand = newOnHand });

        var evt = new StockReceived
        {
            Quantity = quantity,
            NewOnHand = newOnHand
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleReserveStock(InventoryState state, string orderId, int quantity)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Inventory not initialized");

        if (string.IsNullOrWhiteSpace(orderId))
            throw CommandValidationException.InvalidArgument("Order ID is required");

        if (quantity <= 0)
            throw CommandValidationException.InvalidArgument("Quantity must be positive");

        if (state.Reservations.Any(r => r.OrderId == orderId))
            throw CommandValidationException.FailedPrecondition("Reservation already exists for order");

        if (quantity > state.Available)
            throw CommandValidationException.FailedPrecondition($"Insufficient stock: available={state.Available}, requested={quantity}");

        var newReserved = state.Reserved + quantity;
        var newAvailable = state.OnHand - newReserved;

        _logger.Information("reserving_stock {@Data}", new { product_id = state.ProductId, order_id = orderId, quantity, new_available = newAvailable });

        var events = new List<IMessage>();

        events.Add(new StockReserved
        {
            OrderId = orderId,
            Quantity = quantity,
            NewReserved = newReserved
        });

        if (newAvailable < LowStockThreshold && state.Available >= LowStockThreshold)
        {
            events.Add(new LowStockAlert
            {
                ProductId = state.ProductId,
                AvailableQuantity = newAvailable,
                Threshold = LowStockThreshold
            });
        }

        return CreateEventBook(events);
    }

    public EventBook HandleReleaseReservation(InventoryState state, string orderId)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Inventory not initialized");

        var reservation = state.Reservations.FirstOrDefault(r => r.OrderId == orderId)
            ?? throw CommandValidationException.FailedPrecondition("Reservation not found");

        var newReserved = state.Reserved - reservation.Quantity;

        _logger.Information("releasing_reservation {@Data}", new { product_id = state.ProductId, order_id = orderId, quantity = reservation.Quantity, new_reserved = newReserved });

        var evt = new ReservationReleased
        {
            OrderId = orderId,
            Quantity = reservation.Quantity,
            NewReserved = newReserved
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleCommitReservation(InventoryState state, string orderId)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Inventory not initialized");

        var reservation = state.Reservations.FirstOrDefault(r => r.OrderId == orderId)
            ?? throw CommandValidationException.FailedPrecondition("Reservation not found");

        var newOnHand = state.OnHand - reservation.Quantity;
        var newReserved = state.Reserved - reservation.Quantity;

        _logger.Information("committing_reservation {@Data}", new { product_id = state.ProductId, order_id = orderId, quantity = reservation.Quantity, new_on_hand = newOnHand });

        var evt = new ReservationCommitted
        {
            OrderId = orderId,
            Quantity = reservation.Quantity,
            NewOnHand = newOnHand,
            NewReserved = newReserved
        };

        return CreateEventBook(evt);
    }

    private static EventBook CreateEventBook(IMessage evt)
    {
        return CreateEventBook(new List<IMessage> { evt });
    }

    private static EventBook CreateEventBook(List<IMessage> events)
    {
        var book = new EventBook();

        for (var i = 0; i < events.Count; i++)
        {
            var page = new EventPage
            {
                Num = i,
                Event = Any.Pack(events[i]),
                CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
            };
            book.Pages.Add(page);
        }

        return book;
    }
}
