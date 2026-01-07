//! Common event logging utilities for projectors.

use prost::Message;

pub mod proto;
use proto::{
    CustomerCreated, DiscountApplied, LoyaltyPointsAdded, LoyaltyPointsRedeemed,
    TransactionCancelled, TransactionCompleted, TransactionCreated,
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

    // Header
    println!();
    println!("{BOLD}{}{RESET}", "─".repeat(60));
    println!(
        "{BOLD}{}[{}]{RESET} {DIM}seq:{}{RESET}  {CYAN}{}...{RESET}",
        domain_color(domain),
        domain.to_uppercase(),
        sequence,
        root_id
    );
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
        "TransactionCreated" => {
            if let Ok(event) = TransactionCreated::decode(data) {
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
        "DiscountApplied" => {
            if let Ok(event) = DiscountApplied::decode(data) {
                println!("  {DIM}type:{RESET}     {}", event.discount_type);
                println!("  {DIM}value:{RESET}    {}", event.value);
                println!(
                    "  {DIM}discount:{RESET} -${:.2}",
                    event.discount_cents as f64 / 100.0
                );
                if !event.coupon_code.is_empty() {
                    println!("  {DIM}coupon:{RESET}   {}", event.coupon_code);
                }
            }
        }
        "TransactionCompleted" => {
            if let Ok(event) = TransactionCompleted::decode(data) {
                println!(
                    "  {DIM}total:{RESET}    ${:.2}",
                    event.final_total_cents as f64 / 100.0
                );
                println!("  {DIM}payment:{RESET}  {}", event.payment_method);
                println!("  {DIM}loyalty:{RESET}  +{} pts", event.loyalty_points_earned);
            }
        }
        "TransactionCancelled" => {
            if let Ok(event) = TransactionCancelled::decode(data) {
                println!("  {DIM}reason:{RESET} {}", event.reason);
            }
        }
        _ => {
            println!("  {DIM}(raw bytes: {} bytes){RESET}", data.len());
        }
    }
}

fn format_timestamp(ts: &prost_types::Timestamp) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let duration = Duration::new(ts.seconds as u64, ts.nanos as u32);
    let datetime = UNIX_EPOCH + duration;
    format!("{:?}", datetime)
}
