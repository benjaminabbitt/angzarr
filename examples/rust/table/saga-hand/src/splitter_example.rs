//! Saga splitter pattern example for documentation.
//!
//! Demonstrates the splitter pattern where one event triggers commands
//! to multiple different aggregates.

use angzarr_client::proto::angzarr::{CommandBook, CommandPage, Cover, Uuid};
use angzarr_client::proto::examples::{TableSettled, TransferFunds};
use angzarr_client::SagaContext;
use prost_types::Any;

// docs:start:saga_splitter
fn handle_table_settled(event: &TableSettled, context: &SagaContext) -> Vec<CommandBook> {
    // Split one event into commands for multiple player aggregates
    event.payouts.iter().map(|payout| {
        let cmd = TransferFunds {
            table_root: event.table_root.clone(),
            amount: payout.amount.clone(),
        };

        let target_seq = context.get_sequence("player", &payout.player_root);

        CommandBook {
            cover: Some(Cover {
                domain: "player".into(),
                root: Some(Uuid { value: payout.player_root.clone() }),
                ..Default::default()
            }),
            pages: vec![CommandPage {
                sequence: Some(angzarr_client::proto::angzarr::command_page::Sequence::Num(target_seq)),
                command: Some(Any::from_msg(&cmd).unwrap()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }).collect() // One CommandBook per player
}
// docs:end:saga_splitter
