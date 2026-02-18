//! CloudEvents projector - publishes player events as CloudEvents.
//!
//! This projector transforms internal domain events into CloudEvents 1.0 format
//! for external consumption via HTTP webhooks or Kafka.

use angzarr_client::proto::angzarr::CloudEvent;
use angzarr_client::proto::examples::{
    FundsDeposited, PlayerRegistered, PublicFundsDeposited, PublicPlayerRegistered,
};
use angzarr_client::{CloudEventsProjector, CloudEventsRouter};
use prost_types::Any;

// docs:start:cloudevents_oo
pub struct PlayerCloudEventsProjector;

impl CloudEventsProjector for PlayerCloudEventsProjector {
    fn name(&self) -> &str { "prj-player-cloudevents" }
    fn domain(&self) -> &str { "player" }
}

impl PlayerCloudEventsProjector {
    pub fn on_player_registered(&self, event: &PlayerRegistered) -> Option<CloudEvent> {
        // Filter sensitive fields, return public version
        let public = PublicPlayerRegistered {
            display_name: event.display_name.clone(),
            player_type: event.player_type,
        };
        Some(CloudEvent {
            r#type: "com.poker.player.registered".into(),
            data: Some(Any::from_msg(&public).ok()?),
            ..Default::default()
        })
    }

    pub fn on_funds_deposited(&self, event: &FundsDeposited) -> Option<CloudEvent> {
        let public = PublicFundsDeposited {
            amount: event.amount.clone(),
        };
        Some(CloudEvent {
            r#type: "com.poker.player.deposited".into(),
            data: Some(Any::from_msg(&public).ok()?),
            extensions: [("priority".into(), "normal".into())].into(),
            ..Default::default()
        })
    }
}
// docs:end:cloudevents_oo

// docs:start:cloudevents_router
fn handle_player_registered(event: &PlayerRegistered) -> Option<CloudEvent> {
    let public = PublicPlayerRegistered {
        display_name: event.display_name.clone(),
        player_type: event.player_type,
    };
    Some(CloudEvent {
        r#type: "com.poker.player.registered".into(),
        data: Some(Any::from_msg(&public).ok()?),
        ..Default::default()
    })
}

fn handle_funds_deposited(event: &FundsDeposited) -> Option<CloudEvent> {
    let public = PublicFundsDeposited {
        amount: event.amount.clone(),
    };
    Some(CloudEvent {
        r#type: "com.poker.player.deposited".into(),
        data: Some(Any::from_msg(&public).ok()?),
        extensions: [("priority".into(), "normal".into())].into(),
        ..Default::default()
    })
}

fn build_router() -> CloudEventsRouter {
    CloudEventsRouter::new("prj-player-cloudevents", "player")
        .on::<PlayerRegistered>(handle_player_registered)
        .on::<FundsDeposited>(handle_funds_deposited)
}
// docs:end:cloudevents_router

#[tokio::main]
async fn main() {
    let router = build_router();
    angzarr_client::run_cloudevents_projector("prj-player-cloudevents", 50091, router)
        .await
        .expect("CloudEvents projector failed");
}
