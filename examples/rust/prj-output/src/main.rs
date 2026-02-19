//! Projector: Output
//!
//! Subscribes to player, table, and hand domain events.
//! Writes formatted game logs to a file.

use angzarr_client::proto::examples::{
    ActionTaken, BlindPosted, CardsDealt, FundsDeposited, HandComplete, HandStarted,
    PlayerJoined, PlayerRegistered, PotAwarded, TableCreated,
};
use angzarr_client::proto::{event_page, EventBook, Projection};
use angzarr_client::{run_projector_server, ProjectorHandler};
use prost::Message;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;
use tonic::Status;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

fn get_log_file() -> std::io::Result<std::fs::File> {
    let path = env::var("HAND_LOG_FILE").unwrap_or_else(|_| "hand_log.txt".to_string());
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
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

fn get_sequence(page: &angzarr_client::proto::EventPage) -> u32 {
    match &page.sequence {
        Some(event_page::Sequence::Num(n)) => *n,
        _ => 0,
    }
}

// docs:start:projector_functional
fn handle_events(events: &EventBook) -> Result<Projection, Status> {
    let cover = events.cover.as_ref();
    let domain = cover.map(|c| c.domain.as_str()).unwrap_or("");
    let root_id = cover
        .and_then(|c| c.root.as_ref())
        .map(|u| {
            if u.value.len() >= 4 {
                hex::encode(&u.value[..4])
            } else {
                hex::encode(&u.value)
            }
        })
        .unwrap_or_default();

    let mut seq = 0u32;

    for page in &events.pages {
        let event_any = match &page.event {
            Some(e) => e,
            None => continue,
        };
        seq = get_sequence(page);

        let type_url = &event_any.type_url;
        let type_name = type_url
            .rsplit('.')
            .next()
            .unwrap_or(type_url);

        let msg = format_event(domain, &root_id, type_name, &event_any.value);
        write_log(&msg);
    }

    Ok(Projection {
        cover: cover.cloned(),
        projector: "output".to_string(),
        sequence: seq,
        projection: None,
    })
}
// docs:end:projector_functional

fn format_event(domain: &str, root_id: &str, type_name: &str, data: &[u8]) -> String {
    match type_name {
        "PlayerRegistered" => {
            if let Ok(e) = PlayerRegistered::decode(data) {
                return format!(
                    "PLAYER {} registered: {} ({})",
                    root_id, e.display_name, e.email
                );
            }
        }
        "FundsDeposited" => {
            if let Ok(e) = FundsDeposited::decode(data) {
                let amount = e.amount.as_ref().map(|a| a.amount).unwrap_or(0);
                let new_balance = e.new_balance.as_ref().map(|b| b.amount).unwrap_or(0);
                return format!(
                    "PLAYER {} deposited {}, balance: {}",
                    root_id, amount, new_balance
                );
            }
        }
        "TableCreated" => {
            if let Ok(e) = TableCreated::decode(data) {
                return format!(
                    "TABLE {} created: {} ({:?})",
                    root_id, e.table_name, e.game_variant
                );
            }
        }
        "PlayerJoined" => {
            if let Ok(e) = PlayerJoined::decode(data) {
                let player_id = truncate_id(&e.player_root);
                return format!(
                    "TABLE {} player {} joined with {} chips",
                    root_id, player_id, e.stack
                );
            }
        }
        "HandStarted" => {
            if let Ok(e) = HandStarted::decode(data) {
                return format!(
                    "TABLE {} hand #{} started, {} players, dealer at position {}",
                    root_id,
                    e.hand_number,
                    e.active_players.len(),
                    e.dealer_position
                );
            }
        }
        "CardsDealt" => {
            if let Ok(e) = CardsDealt::decode(data) {
                return format!(
                    "HAND {} cards dealt to {} players",
                    root_id,
                    e.player_cards.len()
                );
            }
        }
        "BlindPosted" => {
            if let Ok(e) = BlindPosted::decode(data) {
                let player_id = truncate_id(&e.player_root);
                return format!(
                    "HAND {} player {} posted {} blind: {}",
                    root_id, player_id, e.blind_type, e.amount
                );
            }
        }
        "ActionTaken" => {
            if let Ok(e) = ActionTaken::decode(data) {
                let player_id = truncate_id(&e.player_root);
                return format!(
                    "HAND {} player {}: {:?} {}",
                    root_id, player_id, e.action, e.amount
                );
            }
        }
        "PotAwarded" => {
            if let Ok(e) = PotAwarded::decode(data) {
                let winners: Vec<String> = e
                    .winners
                    .iter()
                    .map(|w| format!("{} wins {}", truncate_id(&w.player_root), w.amount))
                    .collect();
                return format!("HAND {} pot awarded: {}", root_id, winners.join(", "));
            }
        }
        "HandComplete" => {
            if let Ok(e) = HandComplete::decode(data) {
                return format!("HAND {} #{} complete", root_id, e.hand_number);
            }
        }
        _ => {}
    }

    format!("{}.{} [{}]", domain, type_name, root_id)
}

fn truncate_id(id: &[u8]) -> String {
    if id.len() >= 4 {
        hex::encode(&id[..4])
    } else {
        hex::encode(id)
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Clear log file at startup
    let path = env::var("HAND_LOG_FILE").unwrap_or_else(|_| "hand_log.txt".to_string());
    let _ = std::fs::remove_file(&path);

    let handler = ProjectorHandler::new("output").with_handle(handle_events);

    run_projector_server("output", 50090, handler)
        .await
        .expect("Server failed");
}
