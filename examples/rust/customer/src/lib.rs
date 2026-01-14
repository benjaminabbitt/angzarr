//! Customer bounded context business logic.
//!
//! Handles customer lifecycle and loyalty points management.

use async_trait::async_trait;
use prost::Message;

use angzarr::interfaces::business_client::{BusinessError, BusinessLogicClient, Result};
use angzarr::proto::{
    business_response, event_page::Sequence, BusinessResponse, CommandBook, ContextualCommand,
    EventBook, EventPage,
};
use common::proto::{
    AddLoyaltyPoints, CreateCustomer, CustomerCreated, CustomerState, LoyaltyPointsAdded,
    LoyaltyPointsRedeemed, RedeemLoyaltyPoints,
};

pub mod errmsg {
    pub const CUSTOMER_EXISTS: &str = "Customer already exists";
    pub const CUSTOMER_NOT_FOUND: &str = "Customer does not exist";
    pub const NAME_REQUIRED: &str = "Customer name is required";
    pub const EMAIL_REQUIRED: &str = "Customer email is required";
    pub const POINTS_POSITIVE: &str = "Points must be positive";
    pub const INSUFFICIENT_POINTS: &str = "Insufficient points";
    pub const UNKNOWN_COMMAND: &str = "Unknown command type";
    pub const NO_COMMAND_PAGES: &str = "CommandBook has no pages";
}

/// Business logic for Customer aggregate.
pub struct CustomerLogic {
    domain: String,
}

impl CustomerLogic {
    pub const DOMAIN: &'static str = "customer";

    pub fn new() -> Self {
        Self {
            domain: Self::DOMAIN.to_string(),
        }
    }

    /// Rebuild customer state from events.
    fn rebuild_state(&self, event_book: Option<&EventBook>) -> CustomerState {
        let mut state = CustomerState::default();

        let Some(book) = event_book else {
            return state;
        };

        // Start from snapshot if present
        if let Some(snapshot) = &book.snapshot {
            if let Some(snapshot_state) = &snapshot.state {
                if let Ok(s) = CustomerState::decode(snapshot_state.value.as_slice()) {
                    state = s;
                }
            }
        }

        // Apply events
        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };

            if event.type_url.ends_with("CustomerCreated") {
                if let Ok(e) = CustomerCreated::decode(event.value.as_slice()) {
                    state.name = e.name;
                    state.email = e.email;
                }
            } else if event.type_url.ends_with("LoyaltyPointsAdded") {
                if let Ok(e) = LoyaltyPointsAdded::decode(event.value.as_slice()) {
                    state.loyalty_points = e.new_balance;
                    state.lifetime_points += e.points;
                }
            } else if event.type_url.ends_with("LoyaltyPointsRedeemed") {
                if let Ok(e) = LoyaltyPointsRedeemed::decode(event.value.as_slice()) {
                    state.loyalty_points = e.new_balance;
                }
            }
        }

        state
    }

    fn handle_create_customer(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &CustomerState,
    ) -> Result<EventBook> {
        if !state.name.is_empty() {
            return Err(BusinessError::Rejected(errmsg::CUSTOMER_EXISTS.to_string()));
        }

        let cmd = CreateCustomer::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.name.is_empty() {
            return Err(BusinessError::Rejected(errmsg::NAME_REQUIRED.to_string()));
        }
        if cmd.email.is_empty() {
            return Err(BusinessError::Rejected(errmsg::EMAIL_REQUIRED.to_string()));
        }

        let event = CustomerCreated {
            name: cmd.name.clone(),
            email: cmd.email.clone(),
            created_at: Some(now()),
        };

        // New state after applying event
        let new_state = CustomerState {
            name: cmd.name,
            email: cmd.email,
            loyalty_points: 0,
            lifetime_points: 0,
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Force(true)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.CustomerCreated".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.CustomerState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_add_loyalty_points(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &CustomerState,
    ) -> Result<EventBook> {
        if state.name.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::CUSTOMER_NOT_FOUND.to_string(),
            ));
        }

        let cmd = AddLoyaltyPoints::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.points <= 0 {
            return Err(BusinessError::Rejected(errmsg::POINTS_POSITIVE.to_string()));
        }

        let new_balance = state.loyalty_points + cmd.points;

        let event = LoyaltyPointsAdded {
            points: cmd.points,
            new_balance,
            reason: cmd.reason,
        };

        // New state after applying event
        let new_state = CustomerState {
            name: state.name.clone(),
            email: state.email.clone(),
            loyalty_points: new_balance,
            lifetime_points: state.lifetime_points + cmd.points,
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Force(true)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.LoyaltyPointsAdded".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.CustomerState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_redeem_loyalty_points(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &CustomerState,
    ) -> Result<EventBook> {
        if state.name.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::CUSTOMER_NOT_FOUND.to_string(),
            ));
        }

        let cmd = RedeemLoyaltyPoints::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.points <= 0 {
            return Err(BusinessError::Rejected(errmsg::POINTS_POSITIVE.to_string()));
        }
        if cmd.points > state.loyalty_points {
            return Err(BusinessError::Rejected(format!(
                "{}: have {}, need {}",
                errmsg::INSUFFICIENT_POINTS,
                state.loyalty_points,
                cmd.points
            )));
        }

        let new_balance = state.loyalty_points - cmd.points;

        let event = LoyaltyPointsRedeemed {
            points: cmd.points,
            new_balance,
            redemption_type: cmd.redemption_type,
        };

        // New state after applying event (lifetime_points unchanged on redemption)
        let new_state = CustomerState {
            name: state.name.clone(),
            email: state.email.clone(),
            loyalty_points: new_balance,
            lifetime_points: state.lifetime_points,
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Force(true)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.LoyaltyPointsRedeemed".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.CustomerState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }
}

impl Default for CustomerLogic {
    fn default() -> Self {
        Self::new()
    }
}

// Public test methods for cucumber tests
impl CustomerLogic {
    /// Public access to rebuild_state for testing.
    pub fn rebuild_state_public(&self, event_book: Option<&EventBook>) -> CustomerState {
        self.rebuild_state(event_book)
    }

    /// Public access to handle_create_customer for testing.
    pub fn handle_create_customer_public(
        &self,
        command_book: &CommandBook,
        state: &CustomerState,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_create_customer(command_book, &command_any.value, state)
    }

    /// Public access to handle_add_loyalty_points for testing.
    pub fn handle_add_loyalty_points_public(
        &self,
        command_book: &CommandBook,
        state: &CustomerState,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_add_loyalty_points(command_book, &command_any.value, state)
    }

    /// Public access to handle_redeem_loyalty_points for testing.
    pub fn handle_redeem_loyalty_points_public(
        &self,
        command_book: &CommandBook,
        state: &CustomerState,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_redeem_loyalty_points(command_book, &command_any.value, state)
    }
}

#[async_trait]
impl BusinessLogicClient for CustomerLogic {
    async fn handle(&self, _domain: &str, cmd: ContextualCommand) -> Result<BusinessResponse> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = self.rebuild_state(prior_events);

        let Some(cb) = command_book else {
            return Err(BusinessError::Rejected(
                errmsg::NO_COMMAND_PAGES.to_string(),
            ));
        };

        let command_page = cb
            .pages
            .first()
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;

        let command_any = command_page
            .command
            .as_ref()
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;

        let events = if command_any.type_url.ends_with("CreateCustomer") {
            self.handle_create_customer(cb, &command_any.value, &state)?
        } else if command_any.type_url.ends_with("AddLoyaltyPoints") {
            self.handle_add_loyalty_points(cb, &command_any.value, &state)?
        } else if command_any.type_url.ends_with("RedeemLoyaltyPoints") {
            self.handle_redeem_loyalty_points(cb, &command_any.value, &state)?
        } else {
            return Err(BusinessError::Rejected(format!(
                "{}: {}",
                errmsg::UNKNOWN_COMMAND,
                command_any.type_url
            )));
        };

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(events)),
        })
    }

    fn has_domain(&self, domain: &str) -> bool {
        domain == self.domain
    }

    fn domains(&self) -> Vec<String> {
        vec![self.domain.clone()]
    }
}

fn now() -> prost_types::Timestamp {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    prost_types::Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{CommandPage, Cover, Uuid as ProtoUuid};

    fn make_command_book(domain: &str, root: &[u8], type_url: &str, value: Vec<u8>) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.to_vec(),
                }),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                synchronous: false,
                command: Some(prost_types::Any {
                    type_url: type_url.to_string(),
                    value,
                }),
            }],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        }
    }

    fn extract_events(response: BusinessResponse) -> EventBook {
        match response.result {
            Some(business_response::Result::Events(events)) => events,
            _ => panic!("Expected events in response"),
        }
    }

    #[tokio::test]
    async fn test_create_customer_success() {
        let logic = CustomerLogic::new();

        let cmd = CreateCustomer {
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };

        let command_book = make_command_book(
            "customer",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            "type.examples/examples.CreateCustomer",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let response = logic.handle("customer", ctx).await.unwrap();
        let result = extract_events(response);
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
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let cmd = CreateCustomer {
            name: "New".to_string(),
            email: "new@example.com".to_string(),
        };

        let command_book = make_command_book(
            "customer",
            &[1; 16],
            "type.examples/examples.CreateCustomer",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let result = logic.handle("customer", ctx).await;
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
                synchronous: false,
            }],
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let cmd = AddLoyaltyPoints {
            points: 100,
            reason: "transaction:abc123".to_string(),
        };

        let command_book = make_command_book(
            "customer",
            &[1; 16],
            "type.examples/examples.AddLoyaltyPoints",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let response = logic.handle("customer", ctx).await.unwrap();
        let result = extract_events(response);
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

        let command_book = make_command_book(
            "customer",
            &[1; 16],
            "type.examples/examples.AddLoyaltyPoints",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let result = logic.handle("customer", ctx).await;
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
                    synchronous: false,
                },
                EventPage {
                    sequence: Some(Sequence::Num(1)),
                    event: Some(prost_types::Any {
                        type_url: "type.examples/examples.LoyaltyPointsAdded".to_string(),
                        value: LoyaltyPointsAdded {
                            points: 50,
                            new_balance: 50,
                            reason: "test".to_string(),
                        }
                        .encode_to_vec(),
                    }),
                    created_at: None,
                    synchronous: false,
                },
            ],
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let cmd = RedeemLoyaltyPoints {
            points: 100,
            redemption_type: "discount".to_string(),
        };

        let command_book = make_command_book(
            "customer",
            &[1; 16],
            "type.examples/examples.RedeemLoyaltyPoints",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let result = logic.handle("customer", ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient"));
    }
}
