//! Proto type name utilities for extracting type names from proto messages.
//!
//! Provides a trait and macro for deriving type names from proto-generated structs,
//! eliminating hardcoded strings in router registrations and type URL construction.

/// Type URL prefix for example proto types.
pub const TYPE_URL_PREFIX: &str = "type.examples/examples.";

/// Trait for proto messages that provides type name metadata.
///
/// Implemented via the [`impl_proto_type_name!`] macro.
pub trait ProtoTypeName {
    /// The short type name (e.g., "CreateOrder").
    const TYPE_NAME: &'static str;

    /// Build the full type URL for this message type.
    fn type_url() -> String {
        format!("{}{}", TYPE_URL_PREFIX, Self::TYPE_NAME)
    }
}

/// Implement [`ProtoTypeName`] for multiple proto-generated types.
///
/// Takes a list of type identifiers and generates implementations using
/// `stringify!` to derive the type name from the struct name.
///
/// # Example
///
/// ```ignore
/// use common::{impl_proto_type_name, proto::*};
///
/// impl_proto_type_name!(
///     // Commands
///     CreateOrder,
///     ApplyLoyaltyDiscount,
///     // Events
///     OrderCreated,
///     OrderCompleted,
/// );
///
/// // Now you can use:
/// // CreateOrder::TYPE_NAME -> "CreateOrder"
/// // CreateOrder::type_url() -> "type.examples/examples.CreateOrder"
/// ```
#[macro_export]
macro_rules! impl_proto_type_name {
    ($($type:ident),* $(,)?) => {
        $(
            impl $crate::proto_name::ProtoTypeName for $crate::proto::$type {
                const TYPE_NAME: &'static str = stringify!($type);
            }
        )*
    };
}

// Implement ProtoTypeName for all example proto types
impl_proto_type_name!(
    // Order domain - Commands
    CreateOrder,
    ApplyLoyaltyDiscount,
    SubmitPayment,
    ConfirmPayment,
    CancelOrder,
    // Order domain - Events
    OrderCreated,
    LoyaltyDiscountApplied,
    PaymentSubmitted,
    OrderCompleted,
    OrderCancelled,
    // Order domain - State
    OrderState,
    // Fulfillment domain - Commands
    CreateShipment,
    MarkPicked,
    MarkPacked,
    Ship,
    RecordDelivery,
    // Fulfillment domain - Events
    ShipmentCreated,
    ItemsPicked,
    ItemsPacked,
    Shipped,
    Delivered,
    // Fulfillment domain - State
    FulfillmentState,
    // Inventory domain - Commands
    InitializeStock,
    ReceiveStock,
    ReserveStock,
    ReleaseReservation,
    CommitReservation,
    // Inventory domain - Events
    StockInitialized,
    StockReceived,
    StockReserved,
    ReservationReleased,
    ReservationCommitted,
    LowStockAlert,
    // Inventory domain - State
    InventoryState,
    // Shared types
    LineItem,
    // Projections
    Receipt,
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{CreateOrder, OrderCreated};

    #[test]
    fn test_type_name() {
        assert_eq!(CreateOrder::TYPE_NAME, "CreateOrder");
        assert_eq!(OrderCreated::TYPE_NAME, "OrderCreated");
    }

    #[test]
    fn test_type_url() {
        assert_eq!(
            CreateOrder::type_url(),
            "type.examples/examples.CreateOrder"
        );
        assert_eq!(
            OrderCreated::type_url(),
            "type.examples/examples.OrderCreated"
        );
    }
}
