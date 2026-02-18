//! Player Aggregate using OO-style proc macros.
//!
//! This example demonstrates the OO pattern using:
//! - `#[aggregate(domain = "...")]` on impl blocks
//! - `#[handles(CommandType)]` on handler methods
//! - `#[rejected(domain = "...", command = "...")]` on rejection handlers

use angzarr::proto::angzarr::{BusinessResponse, CommandBook, EventBook, EventPage, Notification};
use angzarr::proto::examples::{
    DepositFunds, FundsDeposited, FundsWithdrawn, PlayerRegistered, RegisterPlayer, WithdrawFunds,
};
use angzarr::{run_aggregate_server, CommandResult};
use angzarr_macros::{aggregate, handles, rejected};
use prost_types::Any;

/// Player aggregate state.
#[derive(Default, Clone)]
pub struct PlayerState {
    pub player_id: String,
    pub name: String,
    pub balance: i64,
    pub exists: bool,
}

/// Player aggregate using OO-style annotations.
pub struct PlayerAggregate;

impl PlayerAggregate {
    /// State type for the router generic parameter.
    pub type State = PlayerState;

    /// Rebuilds state from events.
    pub fn rebuild(events: &EventBook) -> PlayerState {
        let mut state = PlayerState::default();
        for page in &events.pages {
            if let Some(event) = &page.event {
                Self::apply_event(&mut state, event);
            }
        }
        state
    }

    fn apply_event(state: &mut PlayerState, event: &Any) {
        if event.type_url.ends_with("PlayerRegistered") {
            if let Ok(e) = prost::Message::decode::<PlayerRegistered>(event.value.as_slice()) {
                state.player_id = e.player_id;
                state.name = e.name;
                state.exists = true;
            }
        } else if event.type_url.ends_with("FundsDeposited") {
            if let Ok(e) = prost::Message::decode::<FundsDeposited>(event.value.as_slice()) {
                if let Some(amount) = e.amount {
                    state.balance += amount.amount as i64;
                }
            }
        } else if event.type_url.ends_with("FundsWithdrawn") {
            if let Ok(e) = prost::Message::decode::<FundsWithdrawn>(event.value.as_slice()) {
                if let Some(amount) = e.amount {
                    state.balance -= amount.amount as i64;
                }
            }
        }
    }
}

#[aggregate(domain = "player")]
impl PlayerAggregate {
    /// Handle RegisterPlayer command.
    #[handles(RegisterPlayer)]
    pub fn register(
        &self,
        _cb: &CommandBook,
        cmd: RegisterPlayer,
        state: &PlayerState,
        seq: u32,
    ) -> CommandResult<EventBook> {
        if state.exists {
            return Err("player already exists".into());
        }

        let event = PlayerRegistered {
            player_id: cmd.player_id,
            name: cmd.name,
        };

        Ok(EventBook {
            pages: vec![EventPage {
                sequence: seq,
                event: Some(Any {
                    type_url: "type.googleapis.com/examples.PlayerRegistered".into(),
                    value: prost::Message::encode_to_vec(&event),
                }),
            }],
            ..Default::default()
        })
    }

    /// Handle DepositFunds command.
    #[handles(DepositFunds)]
    pub fn deposit(
        &self,
        _cb: &CommandBook,
        cmd: DepositFunds,
        state: &PlayerState,
        seq: u32,
    ) -> CommandResult<EventBook> {
        if !state.exists {
            return Err("player does not exist".into());
        }

        let event = FundsDeposited { amount: cmd.amount };

        Ok(EventBook {
            pages: vec![EventPage {
                sequence: seq,
                event: Some(Any {
                    type_url: "type.googleapis.com/examples.FundsDeposited".into(),
                    value: prost::Message::encode_to_vec(&event),
                }),
            }],
            ..Default::default()
        })
    }

    /// Handle WithdrawFunds command.
    #[handles(WithdrawFunds)]
    pub fn withdraw(
        &self,
        _cb: &CommandBook,
        cmd: WithdrawFunds,
        state: &PlayerState,
        seq: u32,
    ) -> CommandResult<EventBook> {
        if !state.exists {
            return Err("player does not exist".into());
        }

        let amount = cmd.amount.as_ref().map(|a| a.amount as i64).unwrap_or(0);
        if state.balance < amount {
            return Err("insufficient funds".into());
        }

        let event = FundsWithdrawn { amount: cmd.amount };

        Ok(EventBook {
            pages: vec![EventPage {
                sequence: seq,
                event: Some(Any {
                    type_url: "type.googleapis.com/examples.FundsWithdrawn".into(),
                    value: prost::Message::encode_to_vec(&event),
                }),
            }],
            ..Default::default()
        })
    }

    /// Handle payment rejection - release reserved funds.
    #[rejected(domain = "payment", command = "ProcessPayment")]
    pub fn handle_payment_rejected(
        &self,
        _notification: &Notification,
        _state: &PlayerState,
    ) -> CommandResult<BusinessResponse> {
        // For now, delegate to framework
        // In a real implementation, this would emit a FundsReleased event
        Ok(BusinessResponse {
            response: Some(
                angzarr::proto::angzarr::business_response::Response::Revocation(
                    angzarr::proto::angzarr::RevocationResponse {
                        emit_system_revocation: true,
                        reason: "Payment rejected, releasing funds".into(),
                    },
                ),
            ),
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agg = PlayerAggregate;

    // The into_router() method is generated by the #[aggregate] macro
    let router = agg.into_router();

    println!("Starting Player aggregate (OO pattern)");
    println!("Domain: {}", router.domain());

    run_aggregate_server(router, "[::]:50403").await?;

    Ok(())
}
