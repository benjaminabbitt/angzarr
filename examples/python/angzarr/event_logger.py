"""Common event logging utilities for projectors."""

from proto import domains_pb2 as domains


# ANSI color codes
BLUE = "\033[94m"
GREEN = "\033[92m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
MAGENTA = "\033[95m"
RED = "\033[91m"
BOLD = "\033[1m"
DIM = "\033[2m"
RESET = "\033[0m"


def domain_color(domain: str) -> str:
    """Get color for domain."""
    return BLUE if domain == "customer" else MAGENTA


def event_color(event_type: str) -> str:
    """Get color for event type."""
    if "Created" in event_type:
        return GREEN
    elif "Completed" in event_type:
        return CYAN
    elif "Cancelled" in event_type:
        return RED
    elif "Added" in event_type or "Applied" in event_type:
        return YELLOW
    return ""


def log_event(domain: str, root_id: str, sequence: int, type_url: str, data: bytes) -> None:
    """Log a single event with pretty formatting."""
    event_type = type_url.split(".")[-1] if type_url else "Unknown"

    # Header
    print()
    print(f"{BOLD}{'─' * 60}{RESET}")
    print(
        f"{BOLD}{domain_color(domain)}[{domain.upper()}]{RESET} "
        f"{DIM}seq:{sequence}{RESET}  "
        f"{CYAN}{root_id}...{RESET}"
    )
    print(f"{BOLD}{event_color(event_type)}{event_type}{RESET}")
    print(f"{'─' * 60}")

    # Parse and print event details
    _print_event_details(event_type, data)


def _print_event_details(event_type: str, data: bytes) -> None:
    """Parse and print event-specific details."""
    if event_type == "CustomerCreated":
        event = domains.CustomerCreated()
        event.ParseFromString(data)
        print(f"  {DIM}name:{RESET}    {event.name}")
        print(f"  {DIM}email:{RESET}   {event.email}")
        if event.HasField("created_at"):
            ts = event.created_at.ToDatetime()
            print(f"  {DIM}created:{RESET} {ts.isoformat()}")

    elif event_type == "LoyaltyPointsAdded":
        event = domains.LoyaltyPointsAdded()
        event.ParseFromString(data)
        print(f"  {DIM}points:{RESET}      +{event.points}")
        print(f"  {DIM}new_balance:{RESET} {event.new_balance}")
        print(f"  {DIM}reason:{RESET}      {event.reason}")

    elif event_type == "LoyaltyPointsRedeemed":
        event = domains.LoyaltyPointsRedeemed()
        event.ParseFromString(data)
        print(f"  {DIM}points:{RESET}      -{event.points}")
        print(f"  {DIM}new_balance:{RESET} {event.new_balance}")
        print(f"  {DIM}type:{RESET}        {event.redemption_type}")

    elif event_type == "TransactionCreated":
        event = domains.TransactionCreated()
        event.ParseFromString(data)
        print(f"  {DIM}customer:{RESET} {event.customer_id[:16]}...")
        print(f"  {DIM}items:{RESET}")
        for item in event.items:
            line_total = item.quantity * item.unit_price_cents
            print(
                f"    - {item.quantity}x {item.name} "
                f"@ ${item.unit_price_cents / 100:.2f} = ${line_total / 100:.2f}"
            )
        print(f"  {DIM}subtotal:{RESET} ${event.subtotal_cents / 100:.2f}")

    elif event_type == "DiscountApplied":
        event = domains.DiscountApplied()
        event.ParseFromString(data)
        print(f"  {DIM}type:{RESET}     {event.discount_type}")
        print(f"  {DIM}value:{RESET}    {event.value}")
        print(f"  {DIM}discount:{RESET} -${event.discount_cents / 100:.2f}")
        if event.coupon_code:
            print(f"  {DIM}coupon:{RESET}   {event.coupon_code}")

    elif event_type == "TransactionCompleted":
        event = domains.TransactionCompleted()
        event.ParseFromString(data)
        print(f"  {DIM}total:{RESET}    ${event.final_total_cents / 100:.2f}")
        print(f"  {DIM}payment:{RESET}  {event.payment_method}")
        print(f"  {DIM}loyalty:{RESET}  +{event.loyalty_points_earned} pts")

    elif event_type == "TransactionCancelled":
        event = domains.TransactionCancelled()
        event.ParseFromString(data)
        print(f"  {DIM}reason:{RESET} {event.reason}")

    else:
        print(f"  {DIM}(raw bytes: {len(data)} bytes){RESET}")


def project_events(event_book: dict) -> None:
    """Process all events in an event book and log them."""
    cover = event_book.get("cover", {})
    domain = cover.get("domain", "unknown")
    root_id = cover.get("root", {}).get("value", b"").hex()[:16]

    pages = event_book.get("pages", [])
    for page in pages:
        event = page.get("event", {})
        type_url = event.get("type_url", "")
        event_data = event.get("value", b"")
        sequence = page.get("sequence", {}).get("num", 0)

        log_event(domain, root_id, sequence, type_url, event_data)
