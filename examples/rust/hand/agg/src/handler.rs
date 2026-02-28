//! Hand command handler implementing CommandHandlerDomainHandler.

use angzarr_client::proto::{CommandBook, EventBook, Notification};
use angzarr_client::{
    dispatch_command, CommandHandlerDomainHandler, CommandResult, RejectionHandlerResponse, StateRouter,
};
use prost_types::Any;

use crate::handlers;
use crate::state::{HandState, STATE_ROUTER};

/// Hand command handler.
pub struct HandHandler;

impl HandHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HandHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandlerDomainHandler for HandHandler {
    type State = HandState;

    fn command_types(&self) -> Vec<String> {
        vec![
            "DealCards".into(),
            "PostBlind".into(),
            "PlayerAction".into(),
            "DealCommunityCards".into(),
            "RequestDraw".into(),
            "RevealCards".into(),
            "AwardPot".into(),
        ]
    }

    fn state_router(&self) -> &StateRouter<Self::State> {
        &STATE_ROUTER
    }

    fn handle(
        &self,
        cmd: &CommandBook,
        payload: &Any,
        state: &Self::State,
        seq: u32,
    ) -> CommandResult<EventBook> {
        dispatch_command!(payload, cmd, state, seq, {
            "DealCards" => handlers::handle_deal_cards,
            "PostBlind" => handlers::handle_post_blind,
            "PlayerAction" => handlers::handle_player_action,
            "DealCommunityCards" => handlers::handle_deal_community_cards,
            "RequestDraw" => handlers::handle_request_draw,
            "RevealCards" => handlers::handle_reveal_cards,
            "AwardPot" => handlers::handle_award_pot,
        })
    }

    fn on_rejected(
        &self,
        _notification: &Notification,
        _state: &Self::State,
        _target_domain: &str,
        _target_command: &str,
    ) -> CommandResult<RejectionHandlerResponse> {
        // Default: let framework handle
        Ok(RejectionHandlerResponse::default())
    }
}
