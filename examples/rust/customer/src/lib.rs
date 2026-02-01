//! Customer bounded context client logic.
//!
//! Handles customer lifecycle and loyalty points management.

mod handlers;
mod state;

use angzarr::proto::{BusinessResponse, ContextualCommand};
use common::proto::CustomerState;
use common::{dispatch_aggregate, unknown_command, AggregateLogic};

pub use handlers::{
    handle_add_loyalty_points, handle_create_customer, handle_redeem_loyalty_points,
};
pub use state::rebuild_state;

pub mod errmsg {
    pub const CUSTOMER_EXISTS: &str = "Customer already exists";
    pub const CUSTOMER_NOT_FOUND: &str = "Customer does not exist";
    pub const NAME_REQUIRED: &str = "Customer name is required";
    pub const EMAIL_REQUIRED: &str = "Customer email is required";
    pub const POINTS_POSITIVE: &str = "Points must be positive";
    pub const INSUFFICIENT_POINTS: &str = "Insufficient points";
}

/// client logic for Customer aggregate.
pub struct CustomerLogic;

common::define_aggregate!(CustomerLogic, "customer");

common::expose_handlers!(fns, CustomerLogic, CustomerState, rebuild: rebuild_state, [
    (handle_create_customer_public, handle_create_customer),
    (handle_add_loyalty_points_public, handle_add_loyalty_points),
    (handle_redeem_loyalty_points_public, handle_redeem_loyalty_points),
]);

#[tonic::async_trait]
impl AggregateLogic for CustomerLogic {
    async fn handle(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<BusinessResponse, tonic::Status> {
        dispatch_aggregate(cmd, rebuild_state, |cb, command_any, state, next_seq| {
            if command_any.type_url.ends_with("CreateCustomer") {
                handle_create_customer(cb, &command_any.value, state, next_seq)
            } else if command_any.type_url.ends_with("AddLoyaltyPoints") {
                handle_add_loyalty_points(cb, &command_any.value, state, next_seq)
            } else if command_any.type_url.ends_with("RedeemLoyaltyPoints") {
                handle_redeem_loyalty_points(cb, &command_any.value, state, next_seq)
            } else {
                Err(unknown_command(&command_any.type_url))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{event_page::Sequence, Cover, EventBook, EventPage, Uuid as ProtoUuid};
    use common::proto::{
        AddLoyaltyPoints, CreateCustomer, CustomerCreated, LoyaltyPointsAdded, RedeemLoyaltyPoints,
    };
    use common::testing::{extract_response_events, make_test_command_book};
    use prost::Message;

    #[tokio::test]
    async fn test_create_customer_success() {
        let logic = CustomerLogic::new();

        let cmd = CreateCustomer {
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        let command_book = make_test_command_book(
            "customer",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            "type.examples/examples.CreateCustomer",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let response = logic.handle(ctx).await.unwrap();
        let result = extract_response_events(response);
        assert_eq!(result.pages.len(), 1);

        let event =
            CustomerCreated::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
                .unwrap();
        assert_eq!(event.name, "John Doe");
        assert_eq!(event.email, "john@example.com");
    }

    #[tokio::test]
    async fn test_create_customer_already_exists() {
        let logic = CustomerLogic::new();

        // Prior events showing customer already exists
        let prior = EventBook {
            cover: Some(Cover {
                domain: "customer".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.CustomerCreated".to_string(),
                    value: CustomerCreated {
                        name: "Existing".to_string(),
                        email: "existing@example.com".to_string(),
                        created_at: None,
                    }
                    .encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot_state: None,
        };

        let cmd = CreateCustomer {
            name: "New".to_string(),
            email: "new@example.com".to_string(),
        };

        let command_book = make_test_command_book(
            "customer",
            &[1; 16],
            "type.examples/examples.CreateCustomer",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let result = logic.handle(ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_add_loyalty_points_success() {
        let logic = CustomerLogic::new();

        // Prior events showing customer exists
        let prior = EventBook {
            cover: Some(Cover {
                domain: "customer".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.CustomerCreated".to_string(),
                    value: CustomerCreated {
                        name: "John".to_string(),
                        email: "john@example.com".to_string(),
                        created_at: None,
                    }
                    .encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot_state: None,
        };

        let cmd = AddLoyaltyPoints {
            points: 100,
            reason: "transaction:abc123".to_string(),
        };

        let command_book = make_test_command_book(
            "customer",
            &[1; 16],
            "type.examples/examples.AddLoyaltyPoints",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let response = logic.handle(ctx).await.unwrap();
        let result = extract_response_events(response);
        assert_eq!(result.pages.len(), 1);

        let event =
            LoyaltyPointsAdded::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
                .unwrap();
        assert_eq!(event.points, 100);
        assert_eq!(event.new_balance, 100);
    }

    #[tokio::test]
    async fn test_add_loyalty_points_requires_existing_customer() {
        let logic = CustomerLogic::new();

        let cmd = AddLoyaltyPoints {
            points: 100,
            reason: "test".to_string(),
        };

        let command_book = make_test_command_book(
            "customer",
            &[1; 16],
            "type.examples/examples.AddLoyaltyPoints",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let result = logic.handle(ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_redeem_loyalty_points_insufficient() {
        let logic = CustomerLogic::new();

        // Prior events: customer with 50 points
        let prior = EventBook {
            cover: Some(Cover {
                domain: "customer".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![
                EventPage {
                    sequence: Some(Sequence::Num(0)),
                    event: Some(prost_types::Any {
                        type_url: "type.examples/examples.CustomerCreated".to_string(),
                        value: CustomerCreated {
                            name: "John".to_string(),
                            email: "john@example.com".to_string(),
                            created_at: None,
                        }
                        .encode_to_vec(),
                    }),
                    created_at: None,
                },
                EventPage {
                    sequence: Some(Sequence::Num(1)),
                    event: Some(prost_types::Any {
                        type_url: "type.examples/examples.LoyaltyPointsAdded".to_string(),
                        value: LoyaltyPointsAdded {
                            points: 50,
                            new_balance: 50,
                            reason: "test".to_string(),
                            new_lifetime_points: 50,
                        }
                        .encode_to_vec(),
                    }),
                    created_at: None,
                },
            ],
            snapshot_state: None,
        };

        let cmd = RedeemLoyaltyPoints {
            points: 100,
            redemption_type: "discount".to_string(),
        };

        let command_book = make_test_command_book(
            "customer",
            &[1; 16],
            "type.examples/examples.RedeemLoyaltyPoints",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let result = logic.handle(ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient"));
    }
}
