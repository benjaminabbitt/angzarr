"""Receipt formatting utilities."""

from models import TransactionState


def format_receipt(transaction_id: str, state: TransactionState) -> str:
    """Format a human-readable receipt."""
    lines = []

    lines.append("=" * 40)
    lines.append("           RECEIPT")
    lines.append("=" * 40)
    lines.append(f"Transaction: {transaction_id[:16]}...")
    lines.append(f"Customer: {state.customer_id[:16]}..." if state.customer_id else "Customer: N/A")
    lines.append("-" * 40)

    # Items
    for item in state.items:
        line_total = item.quantity * item.unit_price_cents
        lines.append(
            f"{item.quantity} x {item.name} @ ${item.unit_price_cents / 100:.2f} = ${line_total / 100:.2f}"
        )

    lines.append("-" * 40)
    lines.append(f"Subtotal:              ${state.subtotal_cents / 100:.2f}")

    if state.discount_cents > 0:
        lines.append(f"Discount ({state.discount_type}):       -${state.discount_cents / 100:.2f}")

    lines.append("-" * 40)
    lines.append(f"TOTAL:                 ${state.final_total_cents / 100:.2f}")
    lines.append(f"Payment: {state.payment_method}")
    lines.append("-" * 40)
    lines.append(f"Loyalty Points Earned: {state.loyalty_points_earned}")
    lines.append("=" * 40)
    lines.append("     Thank you for your purchase!")
    lines.append("=" * 40)

    return "\n".join(lines)
