//! Projector: Output (OO Pattern)
//!
//! Subscribes to player, table, and hand domain events.
//! Writes formatted game logs to a file.
//!
//! This example demonstrates the OO pattern using:
//! - `#[projector(name = "...")]` on impl blocks
//! - `#[projects(EventType)]` on handler methods

use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

use angzarr_client::proto::examples::{
    ActionTaken, BlindPosted, CardsDealt, FundsDeposited, HandComplete, HandStarted,
    PlayerJoined, PlayerRegistered, PotAwarded, TableCreated,
};
use angzarr_client::proto::Projection;
use angzarr_client::run_projector_server;
#[allow(unused_imports)]
use angzarr_macros::{projector, projects};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

fn get_log_file() -> std::io::Result<File> {
    let path = env::var("HAND_LOG_FILE").unwrap_or_else(|_| "hand_log_oo.txt".to_string());
    OpenOptions::new().create(true).append(true).open(path)
}

fn write_log(msg: &str) {
    if let Ok(mut guard) = LOG_FILE.lock() {
        if guard.is_none() {
            *guard = get_log_file().ok();
        }
        if let Some(file) = guard.as_mut() {
            let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] {}", timestamp, msg);
        }
    }
}

fn truncate_id(id: &[u8]) -> String {
    if id.len() >= 4 {
        hex::encode(&id[..4])
    } else {
        hex::encode(id)
    }
}

// docs:start:projector_oo
/// Output projector using OO-style annotations.
pub struct OutputProjector;

#[projector(name = "output")]
impl OutputProjector {
    #[projects(PlayerRegistered)]
    fn project_registered(&self, event: PlayerRegistered) -> Projection {
        write_log(&format!(
            "PLAYER registered: {} ({})",
            event.display_name, event.email
        ));
        Projection {
            projector: "output".to_string(),
            ..Default::default()
        }
    }

    #[projects(FundsDeposited)]
    fn project_deposited(&self, event: FundsDeposited) -> Projection {
        let amount = event.amount.as_ref().map(|a| a.amount).unwrap_or(0);
        let new_balance = event.new_balance.as_ref().map(|b| b.amount).unwrap_or(0);
        write_log(&format!(
            "PLAYER deposited {}, balance: {}",
            amount, new_balance
        ));
        Projection {
            projector: "output".to_string(),
            ..Default::default()
        }
    }

    #[projects(TableCreated)]
    fn project_table_created(&self, event: TableCreated) -> Projection {
        write_log(&format!(
            "TABLE created: {} ({:?})",
            event.table_name, event.game_variant
        ));
        Projection {
            projector: "output".to_string(),
            ..Default::default()
        }
    }

    #[projects(PlayerJoined)]
    fn project_player_joined(&self, event: PlayerJoined) -> Projection {
        let player_id = truncate_id(&event.player_root);
        write_log(&format!(
            "TABLE player {} joined with {} chips",
            player_id, event.stack
        ));
        Projection {
            projector: "output".to_string(),
            ..Default::default()
        }
    }

    #[projects(HandStarted)]
    fn project_hand_started(&self, event: HandStarted) -> Projection {
        write_log(&format!(
            "TABLE hand #{} started, {} players, dealer at position {}",
            event.hand_number,
            event.active_players.len(),
            event.dealer_position
        ));
        Projection {
            projector: "output".to_string(),
            ..Default::default()
        }
    }

    #[projects(CardsDealt)]
    fn project_cards_dealt(&self, event: CardsDealt) -> Projection {
        write_log(&format!(
            "HAND cards dealt to {} players",
            event.player_cards.len()
        ));
        Projection {
            projector: "output".to_string(),
            ..Default::default()
        }
    }

    #[projects(BlindPosted)]
    fn project_blind_posted(&self, event: BlindPosted) -> Projection {
        let player_id = truncate_id(&event.player_root);
        write_log(&format!(
            "HAND player {} posted {} blind: {}",
            player_id, event.blind_type, event.amount
        ));
        Projection {
            projector: "output".to_string(),
            ..Default::default()
        }
    }

    #[projects(ActionTaken)]
    fn project_action_taken(&self, event: ActionTaken) -> Projection {
        let player_id = truncate_id(&event.player_root);
        write_log(&format!(
            "HAND player {}: {:?} {}",
            player_id, event.action, event.amount
        ));
        Projection {
            projector: "output".to_string(),
            ..Default::default()
        }
    }

    #[projects(PotAwarded)]
    fn project_pot_awarded(&self, event: PotAwarded) -> Projection {
        let winners: Vec<String> = event
            .winners
            .iter()
            .map(|w| format!("{} wins {}", truncate_id(&w.player_root), w.amount))
            .collect();
        write_log(&format!("HAND pot awarded: {}", winners.join(", ")));
        Projection {
            projector: "output".to_string(),
            ..Default::default()
        }
    }

    #[projects(HandComplete)]
    fn project_hand_complete(&self, event: HandComplete) -> Projection {
        write_log(&format!("HAND #{} complete", event.hand_number));
        Projection {
            projector: "output".to_string(),
            ..Default::default()
        }
    }
}
// docs:end:projector_oo

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Clear log file at startup
    let path = env::var("HAND_LOG_FILE").unwrap_or_else(|_| "hand_log_oo.txt".to_string());
    let _ = std::fs::remove_file(&path);

    let projector = OutputProjector;
    let handler = projector.into_handler();

    println!("Starting Output projector (OO pattern)");

    run_projector_server("output", 50091, handler)
        .await
        .expect("Server failed");
}
