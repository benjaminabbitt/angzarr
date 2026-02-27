//! Table command handler implementing CommandHandlerDomainHandler.

use angzarr_client::proto::{CommandBook, EventBook, Notification};
use angzarr_client::{
    dispatch_command, CommandHandlerDomainHandler, CommandResult, RejectionHandlerResponse, StateRouter,
};
use prost_types::Any;

use crate::handlers;
use crate::state::{TableState, STATE_ROUTER};

/// Table command handler.
pub struct TableHandler;

impl TableHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TableHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandlerDomainHandler for TableHandler {
    type State = TableState;

    fn command_types(&self) -> Vec<String> {
        vec![
            "CreateTable".into(),
            "JoinTable".into(),
            "LeaveTable".into(),
            "StartHand".into(),
            "EndHand".into(),
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
            "CreateTable" => handlers::handle_create_table,
            "JoinTable" => handlers::handle_join_table,
            "LeaveTable" => handlers::handle_leave_table,
            "StartHand" => handlers::handle_start_hand,
            "EndHand" => handlers::handle_end_hand,
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
