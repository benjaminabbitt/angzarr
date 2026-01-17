//! Common utilities for Angzarr example implementations.

use angzarr::proto::{event_page::Sequence, EventBook};
use prost::Message;

pub mod identity;
pub mod proto;

/// Get the next sequence number for new events based on prior EventBook state.
///
/// Examines the EventBook to find the highest existing sequence:
/// - If pages exist, uses the last page's sequence + 1
/// - If only snapshot exists, uses snapshot.sequence + 1
/// - If empty/None, returns 0
pub fn next_sequence(event_book: Option<&EventBook>) -> u32 {
    let Some(book) = event_book else {
        return 0;
    };

    // Check last event page first (most recent)
    if let Some(last_page) = book.pages.last() {
        if let Some(Sequence::Num(n)) = &last_page.sequence {
            return n + 1;
        }
    }

    // Fall back to snapshot sequence
    if let Some(snapshot) = &book.snapshot {
        return snapshot.sequence + 1;
    }

    0
}
use proto::{
    CustomerCreated, LoyaltyDiscountApplied, LoyaltyPointsAdded, LoyaltyPointsRedeemed,
    OrderCancelled, OrderCompleted, OrderCreated,
};

// ANSI color codes
pub const BLUE: &str = "\x1b[94m";
pub const GREEN: &str = "\x1b[92m";
pub const YELLOW: &str = "\x1b[93m";
pub const CYAN: &str = "\x1b[96m";
pub const MAGENTA: &str = "\x1b[95m";
pub const RED: &str = "\x1b[91m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const RESET: &str = "\x1b[0m";

/// Get color for a domain.
pub fn domain_color(domain: &str) -> &'static str {
    if domain == "customer" {
        BLUE
    } else {
        MAGENTA
    }
}

/// Get color for an event type.
pub fn event_color(event_type: &str) -> &'static str {
    if event_type.contains("Created") {
        GREEN
    } else if event_type.contains("Completed") {
        CYAN
    } else if event_type.contains("Cancelled") {
        RED
    } else if event_type.contains("Added") || event_type.contains("Applied") {
        YELLOW
    } else {
        ""
    }
}

/// Log a single event with pretty formatting.
pub fn log_event(domain: &str, root_id: &str, sequence: u32, type_url: &str, data: &[u8]) {
    let event_type = type_url.rsplit('.').next().unwrap_or(type_url);

    // Standardized event identifier: bounded_ctx:entity_id:sequence (10-digit zero-padded)
    let event_id = format!("{}:{}:{:010}", domain, root_id, sequence);

    // Header
    println!();
    println!("{BOLD}{}{RESET}", "─".repeat(60));
    println!("{DIM}{}{RESET}", event_id);
    println!("{BOLD}{}{}{RESET}", event_color(event_type), event_type);
    println!("{}", "─".repeat(60));

    // Event-specific details
    print_event_details(event_type, data);
}

/// Parse and print event-specific details.
pub fn print_event_details(event_type: &str, data: &[u8]) {
    match event_type {
        "CustomerCreated" => {
            if let Ok(event) = CustomerCreated::decode(data) {
                println!("  {DIM}name:{RESET}    {}", event.name);
                println!("  {DIM}email:{RESET}   {}", event.email);
                if let Some(ts) = event.created_at {
                    println!("  {DIM}created:{RESET} {}", format_timestamp(&ts));
                }
            }
        }
        "LoyaltyPointsAdded" => {
            if let Ok(event) = LoyaltyPointsAdded::decode(data) {
                println!("  {DIM}points:{RESET}      +{}", event.points);
                println!("  {DIM}new_balance:{RESET} {}", event.new_balance);
                println!("  {DIM}reason:{RESET}      {}", event.reason);
            }
        }
        "LoyaltyPointsRedeemed" => {
            if let Ok(event) = LoyaltyPointsRedeemed::decode(data) {
                println!("  {DIM}points:{RESET}      -{}", event.points);
                println!("  {DIM}new_balance:{RESET} {}", event.new_balance);
                println!("  {DIM}type:{RESET}        {}", event.redemption_type);
            }
        }
        "OrderCreated" => {
            if let Ok(event) = OrderCreated::decode(data) {
                let cust_id = &event.customer_id[..16.min(event.customer_id.len())];
                println!("  {DIM}customer:{RESET} {}...", cust_id);
                println!("  {DIM}items:{RESET}");
                for item in &event.items {
                    let line_total = item.quantity * item.unit_price_cents;
                    println!(
                        "    - {}x {} @ ${:.2} = ${:.2}",
                        item.quantity,
                        item.name,
                        item.unit_price_cents as f64 / 100.0,
                        line_total as f64 / 100.0
                    );
                }
                println!(
                    "  {DIM}subtotal:{RESET} ${:.2}",
                    event.subtotal_cents as f64 / 100.0
                );
            }
        }
        "LoyaltyDiscountApplied" => {
            if let Ok(event) = LoyaltyDiscountApplied::decode(data) {
                println!("  {DIM}points:{RESET}   {}", event.points_used);
                println!(
                    "  {DIM}discount:{RESET} -${:.2}",
                    event.discount_cents as f64 / 100.0
                );
            }
        }
        "OrderCompleted" => {
            if let Ok(event) = OrderCompleted::decode(data) {
                println!(
                    "  {DIM}total:{RESET}    ${:.2}",
                    event.final_total_cents as f64 / 100.0
                );
                println!("  {DIM}payment:{RESET}  {}", event.payment_method);
                println!(
                    "  {DIM}loyalty:{RESET}  +{} pts",
                    event.loyalty_points_earned
                );
            }
        }
        "OrderCancelled" => {
            if let Ok(event) = OrderCancelled::decode(data) {
                println!("  {DIM}reason:{RESET} {}", event.reason);
            }
        }
        _ => {
            println!("  {DIM}(raw bytes: {} bytes){RESET}", data.len());
        }
    }
}

fn format_timestamp(ts: &prost_types::Timestamp) -> String {
    use chrono::{DateTime, Utc};
    DateTime::<Utc>::from_timestamp(ts.seconds, ts.nanos as u32)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| format!("{}s", ts.seconds))
}
