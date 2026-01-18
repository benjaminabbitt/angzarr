//! Database schema definitions for web projections.

use sea_query::Iden;

/// Customer order history.
#[derive(Iden)]
pub enum CustomerOrders {
    Table,
    #[iden = "order_id"]
    OrderId,
    #[iden = "customer_id"]
    CustomerId,
    #[iden = "status"]
    Status,
    #[iden = "subtotal_cents"]
    SubtotalCents,
    #[iden = "discount_cents"]
    DiscountCents,
    #[iden = "total_cents"]
    TotalCents,
    #[iden = "item_count"]
    ItemCount,
    #[iden = "loyalty_points_earned"]
    LoyaltyPointsEarned,
    #[iden = "loyalty_points_used"]
    LoyaltyPointsUsed,
    #[iden = "last_sequence"]
    LastSequence,
    #[iden = "created_at"]
    CreatedAt,
    #[iden = "updated_at"]
    UpdatedAt,
}

/// Order line items.
#[derive(Iden)]
pub enum OrderItems {
    Table,
    #[iden = "order_id"]
    OrderId,
    #[iden = "product_id"]
    ProductId,
    #[iden = "product_name"]
    ProductName,
    #[iden = "quantity"]
    Quantity,
    #[iden = "unit_price_cents"]
    UnitPriceCents,
    #[iden = "line_total_cents"]
    LineTotalCents,
}

/// Product catalog.
#[derive(Iden)]
pub enum ProductCatalog {
    Table,
    #[iden = "product_id"]
    ProductId,
    #[iden = "sku"]
    Sku,
    #[iden = "name"]
    Name,
    #[iden = "description"]
    Description,
    #[iden = "price_cents"]
    PriceCents,
    #[iden = "status"]
    Status,
    #[iden = "last_sequence"]
    LastSequence,
    #[iden = "updated_at"]
    UpdatedAt,
}
