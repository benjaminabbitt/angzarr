using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Serilog;

namespace Angzarr.Examples.Cart;

public class CartLogic : ICartLogic
{
    private readonly Serilog.ILogger _logger;

    public CartLogic(Serilog.ILogger logger)
    {
        _logger = logger.ForContext<CartLogic>();
    }

    public CartState RebuildState(EventBook? eventBook)
    {
        var state = CartState.Empty;

        if (eventBook == null || eventBook.Pages.Count == 0)
            return state;

        foreach (var page in eventBook.Pages)
        {
            if (page.Event == null) continue;
            state = ApplyEvent(state, page.Event);
        }

        return state;
    }

    private CartState ApplyEvent(CartState state, Any eventAny)
    {
        var typeUrl = eventAny.TypeUrl;

        if (typeUrl.EndsWith("CartCreated"))
        {
            var evt = eventAny.Unpack<CartCreated>();
            return state with { CustomerId = evt.CustomerId, Status = "active" };
        }

        if (typeUrl.EndsWith("ItemAdded"))
        {
            var evt = eventAny.Unpack<ItemAdded>();
            var newItems = new List<LineItem>(state.Items) { evt.Item };
            return state with { Items = newItems, SubtotalCents = evt.NewSubtotalCents };
        }

        if (typeUrl.EndsWith("QuantityUpdated"))
        {
            var evt = eventAny.Unpack<QuantityUpdated>();
            var updatedItems = state.Items.Select(item =>
                item.ProductId == evt.ProductId
                    ? new LineItem { ProductId = item.ProductId, Name = item.Name, Quantity = evt.NewQuantity, UnitPriceCents = item.UnitPriceCents }
                    : item).ToList();
            return state with { Items = updatedItems, SubtotalCents = evt.NewSubtotalCents };
        }

        if (typeUrl.EndsWith("ItemRemoved"))
        {
            var evt = eventAny.Unpack<ItemRemoved>();
            var remainingItems = state.Items.Where(i => i.ProductId != evt.ProductId).ToList();
            return state with { Items = remainingItems, SubtotalCents = evt.NewSubtotalCents };
        }

        if (typeUrl.EndsWith("CouponApplied"))
        {
            var evt = eventAny.Unpack<CouponApplied>();
            return state with { CouponCode = evt.CouponCode, DiscountCents = evt.DiscountCents };
        }

        if (typeUrl.EndsWith("CartCleared"))
        {
            return state with { Items = new List<LineItem>(), SubtotalCents = 0, CouponCode = "", DiscountCents = 0 };
        }

        if (typeUrl.EndsWith("CartCheckoutRequested"))
        {
            return state with { Status = "checked_out" };
        }

        return state;
    }

    public EventBook HandleCreateCart(CartState state, string customerId)
    {
        if (state.Exists)
            throw CommandValidationException.FailedPrecondition("Cart already exists");

        if (string.IsNullOrWhiteSpace(customerId))
            throw CommandValidationException.InvalidArgument("Customer ID is required");

        _logger.Information("creating_cart {@Data}", new { customer_id = customerId });

        var evt = new CartCreated
        {
            CustomerId = customerId,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleAddItem(CartState state, string productId, string name, int quantity, int unitPriceCents)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Cart does not exist");

        if (state.IsCheckedOut)
            throw CommandValidationException.FailedPrecondition("Cart already checked out");

        if (quantity <= 0)
            throw CommandValidationException.InvalidArgument("Quantity must be positive");

        if (state.Items.Any(i => i.ProductId == productId))
            throw CommandValidationException.FailedPrecondition("Item already in cart, use UpdateQuantity");

        var item = new LineItem
        {
            ProductId = productId,
            Name = name,
            Quantity = quantity,
            UnitPriceCents = unitPriceCents
        };

        var itemTotal = quantity * unitPriceCents;
        var newSubtotal = state.SubtotalCents + itemTotal;

        _logger.Information("adding_item {@Data}", new { product_id = productId, quantity, new_subtotal = newSubtotal });

        var evt = new ItemAdded
        {
            Item = item,
            NewSubtotalCents = newSubtotal
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleUpdateQuantity(CartState state, string productId, int quantity)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Cart does not exist");

        if (state.IsCheckedOut)
            throw CommandValidationException.FailedPrecondition("Cart already checked out");

        if (quantity <= 0)
            throw CommandValidationException.InvalidArgument("Quantity must be positive");

        var existingItem = state.Items.FirstOrDefault(i => i.ProductId == productId)
            ?? throw CommandValidationException.FailedPrecondition("Item not in cart");

        var oldItemTotal = existingItem.Quantity * existingItem.UnitPriceCents;
        var newItemTotal = quantity * existingItem.UnitPriceCents;
        var newSubtotal = state.SubtotalCents - oldItemTotal + newItemTotal;

        _logger.Information("updating_quantity {@Data}", new { product_id = productId, old_qty = existingItem.Quantity, new_qty = quantity });

        var evt = new QuantityUpdated
        {
            ProductId = productId,
            OldQuantity = existingItem.Quantity,
            NewQuantity = quantity,
            NewSubtotalCents = newSubtotal
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleRemoveItem(CartState state, string productId)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Cart does not exist");

        if (state.IsCheckedOut)
            throw CommandValidationException.FailedPrecondition("Cart already checked out");

        var existingItem = state.Items.FirstOrDefault(i => i.ProductId == productId)
            ?? throw CommandValidationException.FailedPrecondition("Item not in cart");

        var itemTotal = existingItem.Quantity * existingItem.UnitPriceCents;
        var newSubtotal = state.SubtotalCents - itemTotal;

        _logger.Information("removing_item {@Data}", new { product_id = productId, new_subtotal = newSubtotal });

        var evt = new ItemRemoved
        {
            ProductId = productId,
            NewSubtotalCents = newSubtotal
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleApplyCoupon(CartState state, string couponCode, int discountCents)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Cart does not exist");

        if (state.IsCheckedOut)
            throw CommandValidationException.FailedPrecondition("Cart already checked out");

        if (string.IsNullOrWhiteSpace(couponCode))
            throw CommandValidationException.InvalidArgument("Coupon code is required");

        if (!string.IsNullOrEmpty(state.CouponCode))
            throw CommandValidationException.FailedPrecondition("Coupon already applied");

        _logger.Information("applying_coupon {@Data}", new { code = couponCode, discount_cents = discountCents });

        var evt = new CouponApplied
        {
            CouponCode = couponCode,
            DiscountCents = discountCents
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleClearCart(CartState state)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Cart does not exist");

        if (state.IsCheckedOut)
            throw CommandValidationException.FailedPrecondition("Cart already checked out");

        _logger.Information("clearing_cart {@Data}", new { customer_id = state.CustomerId });

        var evt = new CartCleared
        {
            ClearedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        return CreateEventBook(evt);
    }

    public EventBook HandleCheckout(CartState state, int loyaltyPointsToUse)
    {
        if (!state.Exists)
            throw CommandValidationException.FailedPrecondition("Cart does not exist");

        if (state.IsCheckedOut)
            throw CommandValidationException.FailedPrecondition("Cart already checked out");

        if (!state.Items.Any())
            throw CommandValidationException.FailedPrecondition("Cart is empty");

        var totalCents = state.SubtotalCents - state.DiscountCents;

        _logger.Information("checkout_requested {@Data}",
            new { customer_id = state.CustomerId, total_cents = totalCents, loyalty_points = loyaltyPointsToUse });

        var evt = new CartCheckoutRequested
        {
            CustomerId = state.CustomerId,
            SubtotalCents = state.SubtotalCents,
            DiscountCents = state.DiscountCents,
            LoyaltyPointsToUse = loyaltyPointsToUse,
            RequestedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.Items.AddRange(state.Items);

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
