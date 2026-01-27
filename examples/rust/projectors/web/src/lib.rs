//! Web Projector - builds read models for web UI.
//!
//! Tracks orders, order items, and products for customer-facing queries.

mod schema;

use prost::Message;
use sea_query::{ColumnDef, Expr, Index, OnConflict, PostgresQueryBuilder, Query, Table};
use sqlx::{PgPool, Row};
use tracing::{debug, error};

use angzarr::proto::{EventBook, Projection};
use common::proto::{
    LoyaltyDiscountApplied, OrderCancelled, OrderCompleted, OrderCreated, PaymentSubmitted,
    PriceSet, ProductCreated, ProductDiscontinued, ProductUpdated,
};

use schema::{CustomerOrders, OrderItems, ProductCatalog};

/// Errors that can occur during projection.
#[derive(Debug, thiserror::Error)]
pub enum ProjectorError {
    #[error("Storage error: {0}")]
    Storage(String),
}

pub type Result<T> = std::result::Result<T, ProjectorError>;

pub const PROJECTOR_NAME: &str = "web";

/// Order status values.
#[derive(Debug, Clone, Copy)]
pub enum OrderStatus {
    Pending,
    PaymentSubmitted,
    Completed,
    Cancelled,
}

impl OrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderStatus::Pending => "pending",
            OrderStatus::PaymentSubmitted => "payment_submitted",
            OrderStatus::Completed => "completed",
            OrderStatus::Cancelled => "cancelled",
        }
    }
}

/// Product status values.
#[derive(Debug, Clone, Copy)]
pub enum ProductStatus {
    Active,
    Discontinued,
}

impl ProductStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProductStatus::Active => "active",
            ProductStatus::Discontinued => "discontinued",
        }
    }
}

/// Web Projector implementation.
///
/// Writes read models directly to PostgreSQL.
pub struct WebProjector {
    name: String,
    pool: PgPool,
}

impl WebProjector {
    /// Create a new web projector with the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            name: PROJECTOR_NAME.to_string(),
            pool,
        }
    }

    /// Initialize database schema.
    pub async fn init(&self) -> std::result::Result<(), sqlx::Error> {
        // Create customer_orders table
        let create_orders = Table::create()
            .table(CustomerOrders::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(CustomerOrders::OrderId)
                    .text()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(CustomerOrders::CustomerId).text().not_null())
            .col(ColumnDef::new(CustomerOrders::Status).text().not_null())
            .col(
                ColumnDef::new(CustomerOrders::SubtotalCents)
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(CustomerOrders::DiscountCents)
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(CustomerOrders::TotalCents)
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(CustomerOrders::ItemCount)
                    .integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(CustomerOrders::LoyaltyPointsEarned)
                    .integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(CustomerOrders::LoyaltyPointsUsed)
                    .integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(CustomerOrders::LastSequence)
                    .integer()
                    .not_null(),
            )
            .col(ColumnDef::new(CustomerOrders::CreatedAt).text().not_null())
            .col(ColumnDef::new(CustomerOrders::UpdatedAt).text().not_null())
            .to_string(PostgresQueryBuilder);

        sqlx::query(&create_orders).execute(&self.pool).await?;

        // Index on customer_id
        let idx_customer = Index::create()
            .if_not_exists()
            .name("idx_customer_orders_customer")
            .table(CustomerOrders::Table)
            .col(CustomerOrders::CustomerId)
            .to_string(PostgresQueryBuilder);

        sqlx::query(&idx_customer).execute(&self.pool).await?;

        // Index on status
        let idx_status = Index::create()
            .if_not_exists()
            .name("idx_customer_orders_status")
            .table(CustomerOrders::Table)
            .col(CustomerOrders::Status)
            .to_string(PostgresQueryBuilder);

        sqlx::query(&idx_status).execute(&self.pool).await?;

        // Create order_items table
        let create_items = Table::create()
            .table(OrderItems::Table)
            .if_not_exists()
            .col(ColumnDef::new(OrderItems::OrderId).text().not_null())
            .col(ColumnDef::new(OrderItems::ProductId).text().not_null())
            .col(ColumnDef::new(OrderItems::ProductName).text().not_null())
            .col(ColumnDef::new(OrderItems::Quantity).integer().not_null())
            .col(
                ColumnDef::new(OrderItems::UnitPriceCents)
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(OrderItems::LineTotalCents)
                    .big_integer()
                    .not_null(),
            )
            .primary_key(
                Index::create()
                    .col(OrderItems::OrderId)
                    .col(OrderItems::ProductId),
            )
            .to_string(PostgresQueryBuilder);

        sqlx::query(&create_items).execute(&self.pool).await?;

        // Create product_catalog table
        let create_products = Table::create()
            .table(ProductCatalog::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(ProductCatalog::ProductId)
                    .text()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(ProductCatalog::Sku).text().not_null())
            .col(ColumnDef::new(ProductCatalog::Name).text().not_null())
            .col(
                ColumnDef::new(ProductCatalog::Description)
                    .text()
                    .not_null(),
            )
            .col(
                ColumnDef::new(ProductCatalog::PriceCents)
                    .big_integer()
                    .not_null(),
            )
            .col(ColumnDef::new(ProductCatalog::Status).text().not_null())
            .col(
                ColumnDef::new(ProductCatalog::LastSequence)
                    .integer()
                    .not_null(),
            )
            .col(ColumnDef::new(ProductCatalog::UpdatedAt).text().not_null())
            .to_string(PostgresQueryBuilder);

        sqlx::query(&create_products).execute(&self.pool).await?;

        // Unique index on SKU
        let idx_sku = Index::create()
            .if_not_exists()
            .name("idx_product_catalog_sku")
            .table(ProductCatalog::Table)
            .col(ProductCatalog::Sku)
            .unique()
            .to_string(PostgresQueryBuilder);

        sqlx::query(&idx_sku).execute(&self.pool).await?;

        Ok(())
    }

    /// Upsert an order record.
    async fn upsert_order(
        &self,
        order_id: &str,
        customer_id: &str,
        status: OrderStatus,
        subtotal_cents: i64,
        discount_cents: i64,
        total_cents: i64,
        item_count: i32,
        loyalty_points_earned: i32,
        loyalty_points_used: i32,
        sequence: u32,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        let query = Query::insert()
            .into_table(CustomerOrders::Table)
            .columns([
                CustomerOrders::OrderId,
                CustomerOrders::CustomerId,
                CustomerOrders::Status,
                CustomerOrders::SubtotalCents,
                CustomerOrders::DiscountCents,
                CustomerOrders::TotalCents,
                CustomerOrders::ItemCount,
                CustomerOrders::LoyaltyPointsEarned,
                CustomerOrders::LoyaltyPointsUsed,
                CustomerOrders::LastSequence,
                CustomerOrders::CreatedAt,
                CustomerOrders::UpdatedAt,
            ])
            .values_panic([
                order_id.into(),
                customer_id.into(),
                status.as_str().into(),
                subtotal_cents.into(),
                discount_cents.into(),
                total_cents.into(),
                item_count.into(),
                loyalty_points_earned.into(),
                loyalty_points_used.into(),
                (sequence as i32).into(),
                now.clone().into(),
                now.into(),
            ])
            .on_conflict(
                OnConflict::column(CustomerOrders::OrderId)
                    .update_columns([
                        CustomerOrders::Status,
                        CustomerOrders::SubtotalCents,
                        CustomerOrders::DiscountCents,
                        CustomerOrders::TotalCents,
                        CustomerOrders::ItemCount,
                        CustomerOrders::LoyaltyPointsEarned,
                        CustomerOrders::LoyaltyPointsUsed,
                        CustomerOrders::LastSequence,
                        CustomerOrders::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    /// Update order status only.
    async fn update_order_status(
        &self,
        order_id: &str,
        status: OrderStatus,
        sequence: u32,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        let query = Query::update()
            .table(CustomerOrders::Table)
            .values([
                (CustomerOrders::Status, status.as_str().into()),
                (CustomerOrders::LastSequence, (sequence as i32).into()),
                (CustomerOrders::UpdatedAt, now.into()),
            ])
            .and_where(Expr::col(CustomerOrders::OrderId).eq(order_id))
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    /// Update order with completion data.
    async fn complete_order(
        &self,
        order_id: &str,
        total_cents: i64,
        loyalty_points_earned: i32,
        sequence: u32,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        let query = Query::update()
            .table(CustomerOrders::Table)
            .values([
                (
                    CustomerOrders::Status,
                    OrderStatus::Completed.as_str().into(),
                ),
                (CustomerOrders::TotalCents, total_cents.into()),
                (
                    CustomerOrders::LoyaltyPointsEarned,
                    loyalty_points_earned.into(),
                ),
                (CustomerOrders::LastSequence, (sequence as i32).into()),
                (CustomerOrders::UpdatedAt, now.into()),
            ])
            .and_where(Expr::col(CustomerOrders::OrderId).eq(order_id))
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    /// Update order with discount.
    async fn apply_order_discount(
        &self,
        order_id: &str,
        discount_cents: i64,
        points_used: i32,
        sequence: u32,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        // Get current order to calculate new total
        let select = Query::select()
            .columns([CustomerOrders::SubtotalCents, CustomerOrders::DiscountCents])
            .from(CustomerOrders::Table)
            .and_where(Expr::col(CustomerOrders::OrderId).eq(order_id))
            .to_string(PostgresQueryBuilder);

        if let Some(row) = sqlx::query(&select).fetch_optional(&self.pool).await? {
            let subtotal: i64 = row.get("subtotal_cents");
            let existing_discount: i64 = row.get("discount_cents");
            let new_discount = existing_discount + discount_cents;
            let new_total = subtotal - new_discount;

            let query = Query::update()
                .table(CustomerOrders::Table)
                .values([
                    (CustomerOrders::DiscountCents, new_discount.into()),
                    (CustomerOrders::TotalCents, new_total.into()),
                    (CustomerOrders::LoyaltyPointsUsed, points_used.into()),
                    (CustomerOrders::LastSequence, (sequence as i32).into()),
                    (CustomerOrders::UpdatedAt, now.into()),
                ])
                .and_where(Expr::col(CustomerOrders::OrderId).eq(order_id))
                .to_string(PostgresQueryBuilder);

            sqlx::query(&query).execute(&self.pool).await?;
        }

        Ok(())
    }

    /// Set order items.
    async fn set_order_items(
        &self,
        order_id: &str,
        items: &[common::proto::LineItem],
    ) -> std::result::Result<(), sqlx::Error> {
        // Delete existing items
        let delete = Query::delete()
            .from_table(OrderItems::Table)
            .and_where(Expr::col(OrderItems::OrderId).eq(order_id))
            .to_string(PostgresQueryBuilder);

        sqlx::query(&delete).execute(&self.pool).await?;

        // Insert new items
        for item in items {
            let insert = Query::insert()
                .into_table(OrderItems::Table)
                .columns([
                    OrderItems::OrderId,
                    OrderItems::ProductId,
                    OrderItems::ProductName,
                    OrderItems::Quantity,
                    OrderItems::UnitPriceCents,
                    OrderItems::LineTotalCents,
                ])
                .values_panic([
                    order_id.into(),
                    item.product_id.clone().into(),
                    item.name.clone().into(),
                    item.quantity.into(),
                    (item.unit_price_cents as i64).into(),
                    ((item.unit_price_cents * item.quantity) as i64).into(),
                ])
                .to_string(PostgresQueryBuilder);

            sqlx::query(&insert).execute(&self.pool).await?;
        }

        Ok(())
    }

    /// Upsert a product.
    async fn upsert_product(
        &self,
        product_id: &str,
        sku: &str,
        name: &str,
        description: &str,
        price_cents: i64,
        status: ProductStatus,
        sequence: u32,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        let query = Query::insert()
            .into_table(ProductCatalog::Table)
            .columns([
                ProductCatalog::ProductId,
                ProductCatalog::Sku,
                ProductCatalog::Name,
                ProductCatalog::Description,
                ProductCatalog::PriceCents,
                ProductCatalog::Status,
                ProductCatalog::LastSequence,
                ProductCatalog::UpdatedAt,
            ])
            .values_panic([
                product_id.into(),
                sku.into(),
                name.into(),
                description.into(),
                price_cents.into(),
                status.as_str().into(),
                (sequence as i32).into(),
                now.into(),
            ])
            .on_conflict(
                OnConflict::column(ProductCatalog::ProductId)
                    .update_columns([
                        ProductCatalog::Sku,
                        ProductCatalog::Name,
                        ProductCatalog::Description,
                        ProductCatalog::PriceCents,
                        ProductCatalog::Status,
                        ProductCatalog::LastSequence,
                        ProductCatalog::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    /// Update product price.
    async fn update_product_price(
        &self,
        product_id: &str,
        price_cents: i64,
        sequence: u32,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        let query = Query::update()
            .table(ProductCatalog::Table)
            .values([
                (ProductCatalog::PriceCents, price_cents.into()),
                (ProductCatalog::LastSequence, (sequence as i32).into()),
                (ProductCatalog::UpdatedAt, now.into()),
            ])
            .and_where(Expr::col(ProductCatalog::ProductId).eq(product_id))
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    /// Discontinue a product.
    async fn discontinue_product(
        &self,
        product_id: &str,
        sequence: u32,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        let query = Query::update()
            .table(ProductCatalog::Table)
            .values([
                (
                    ProductCatalog::Status,
                    ProductStatus::Discontinued.as_str().into(),
                ),
                (ProductCatalog::LastSequence, (sequence as i32).into()),
                (ProductCatalog::UpdatedAt, now.into()),
            ])
            .and_where(Expr::col(ProductCatalog::ProductId).eq(product_id))
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    /// Process a single event.
    async fn process_event(
        &self,
        event: &prost_types::Any,
        root_id: &str,
        domain: &str,
        sequence: u32,
    ) -> std::result::Result<(), ProjectorError> {
        let type_url = &event.type_url;

        match domain {
            "order" => {
                if type_url.ends_with("OrderCreated") {
                    if let Ok(created) = OrderCreated::decode(event.value.as_slice()) {
                        debug!(order_id = %root_id, "OrderCreated");
                        self.upsert_order(
                            root_id,
                            &created.customer_id,
                            OrderStatus::Pending,
                            created.subtotal_cents as i64,
                            0,
                            created.subtotal_cents as i64,
                            created.items.len() as i32,
                            0,
                            0,
                            sequence,
                        )
                        .await
                        .map_err(|e| ProjectorError::Storage(e.to_string()))?;

                        self.set_order_items(root_id, &created.items)
                            .await
                            .map_err(|e| ProjectorError::Storage(e.to_string()))?;
                    }
                } else if type_url.ends_with("LoyaltyDiscountApplied") {
                    if let Ok(discount) = LoyaltyDiscountApplied::decode(event.value.as_slice()) {
                        self.apply_order_discount(
                            root_id,
                            discount.discount_cents as i64,
                            discount.points_used,
                            sequence,
                        )
                        .await
                        .map_err(|e| ProjectorError::Storage(e.to_string()))?;
                    }
                } else if type_url.ends_with("PaymentSubmitted") {
                    if let Ok(_submitted) = PaymentSubmitted::decode(event.value.as_slice()) {
                        self.update_order_status(root_id, OrderStatus::PaymentSubmitted, sequence)
                            .await
                            .map_err(|e| ProjectorError::Storage(e.to_string()))?;
                    }
                } else if type_url.ends_with("OrderCompleted") {
                    if let Ok(completed) = OrderCompleted::decode(event.value.as_slice()) {
                        self.complete_order(
                            root_id,
                            completed.final_total_cents as i64,
                            completed.loyalty_points_earned,
                            sequence,
                        )
                        .await
                        .map_err(|e| ProjectorError::Storage(e.to_string()))?;
                    }
                } else if type_url.ends_with("OrderCancelled") {
                    if let Ok(_cancelled) = OrderCancelled::decode(event.value.as_slice()) {
                        self.update_order_status(root_id, OrderStatus::Cancelled, sequence)
                            .await
                            .map_err(|e| ProjectorError::Storage(e.to_string()))?;
                    }
                }
            }
            "product" => {
                if type_url.ends_with("ProductCreated") {
                    if let Ok(created) = ProductCreated::decode(event.value.as_slice()) {
                        self.upsert_product(
                            root_id,
                            &created.sku,
                            &created.name,
                            &created.description,
                            created.price_cents as i64,
                            ProductStatus::Active,
                            sequence,
                        )
                        .await
                        .map_err(|e| ProjectorError::Storage(e.to_string()))?;
                    }
                } else if type_url.ends_with("ProductUpdated") {
                    if let Ok(updated) = ProductUpdated::decode(event.value.as_slice()) {
                        // Get existing product to preserve SKU and price
                        let select = Query::select()
                            .columns([ProductCatalog::Sku, ProductCatalog::PriceCents])
                            .from(ProductCatalog::Table)
                            .and_where(Expr::col(ProductCatalog::ProductId).eq(root_id))
                            .to_string(PostgresQueryBuilder);

                        if let Some(row) = sqlx::query(&select)
                            .fetch_optional(&self.pool)
                            .await
                            .map_err(|e| ProjectorError::Storage(e.to_string()))?
                        {
                            let sku: String = row.get("sku");
                            let price: i64 = row.get("price_cents");
                            self.upsert_product(
                                root_id,
                                &sku,
                                &updated.name,
                                &updated.description,
                                price,
                                ProductStatus::Active,
                                sequence,
                            )
                            .await
                            .map_err(|e| ProjectorError::Storage(e.to_string()))?;
                        }
                    }
                } else if type_url.ends_with("PriceSet") {
                    if let Ok(price_set) = PriceSet::decode(event.value.as_slice()) {
                        self.update_product_price(root_id, price_set.price_cents as i64, sequence)
                            .await
                            .map_err(|e| ProjectorError::Storage(e.to_string()))?;
                    }
                } else if type_url.ends_with("ProductDiscontinued") {
                    if let Ok(_discontinued) = ProductDiscontinued::decode(event.value.as_slice()) {
                        self.discontinue_product(root_id, sequence)
                            .await
                            .map_err(|e| ProjectorError::Storage(e.to_string()))?;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl WebProjector {
    /// Handle an event book, projecting events to the read model.
    pub async fn handle(&self, book: &EventBook) -> Result<Option<Projection>> {
        let cover = book.cover.as_ref();
        let domain = cover.map(|c| c.domain.as_str()).unwrap_or("unknown");
        let root_id = cover
            .and_then(|c| c.root.as_ref())
            .map(|r| hex::encode(&r.value))
            .unwrap_or_else(|| "unknown".to_string());

        for page in &book.pages {
            let sequence = match &page.sequence {
                Some(angzarr::proto::event_page::Sequence::Num(n)) => *n,
                _ => 0,
            };

            if let Some(event) = &page.event {
                if let Err(e) = self.process_event(event, &root_id, domain, sequence).await {
                    error!(error = %e, event_type = %event.type_url, "Failed to process event");
                }
            }
        }

        // Web projector doesn't return sync projections
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projector_name() {
        assert_eq!(PROJECTOR_NAME, "web");
    }

    #[test]
    fn test_order_status_as_str() {
        assert_eq!(OrderStatus::Pending.as_str(), "pending");
        assert_eq!(OrderStatus::Completed.as_str(), "completed");
    }

    #[test]
    fn test_product_status_as_str() {
        assert_eq!(ProductStatus::Active.as_str(), "active");
        assert_eq!(ProductStatus::Discontinued.as_str(), "discontinued");
    }
}
