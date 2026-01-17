#include "order_logic.hpp"
#include "validation_error.hpp"
#include <chrono>

namespace order {

using namespace angzarr;

OrderState OrderLogic::rebuild_state(const angzarr::EventBook* event_book) {
    OrderState state;
    if (!event_book || event_book->pages().empty()) return state;

    for (const auto& page : event_book->pages()) {
        if (page.has_event()) {
            state = apply_event(std::move(state), page.event());
        }
    }
    return state;
}

examples::OrderCreated OrderLogic::handle_create_order(
    const OrderState& state, const std::string& customer_id,
    const std::vector<OrderItem>& items, int32_t subtotal_cents,
    int32_t discount_cents, int32_t total_cents) {
    if (state.exists()) throw ValidationError::failed_precondition("Order already exists");
    if (customer_id.empty()) throw ValidationError::invalid_argument("Customer ID is required");
    if (items.empty()) throw ValidationError::invalid_argument("Order must have items");
    if (total_cents <= 0) throw ValidationError::invalid_argument("Total must be positive");

    examples::OrderCreated event;
    event.set_customer_id(customer_id);
    for (const auto& item : items) {
        auto* order_item = event.add_items();
        order_item->set_product_id(item.product_id);
        order_item->set_quantity(item.quantity);
        order_item->set_price_cents(item.price_cents);
    }
    event.set_subtotal_cents(subtotal_cents);
    event.set_discount_cents(discount_cents);
    event.set_total_cents(total_cents);
    event.mutable_created_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

examples::LoyaltyDiscountApplied OrderLogic::handle_apply_loyalty_discount(
    const OrderState& state, int32_t points_used, int32_t discount_cents) {
    if (!state.exists()) throw ValidationError::failed_precondition("Order does not exist");
    if (state.status != OrderStatus::Created) throw ValidationError::failed_precondition("Order not in created state");
    if (points_used <= 0) throw ValidationError::invalid_argument("Points used must be positive");

    examples::LoyaltyDiscountApplied event;
    event.set_points_used(points_used);
    event.set_discount_cents(discount_cents);
    return event;
}

examples::PaymentSubmitted OrderLogic::handle_submit_payment(
    const OrderState& state, const std::string& payment_method,
    const std::string& payment_reference, int32_t amount_cents) {
    if (!state.exists()) throw ValidationError::failed_precondition("Order does not exist");
    if (!state.can_accept_payment()) throw ValidationError::failed_precondition("Order cannot accept payment");
    if (payment_method.empty()) throw ValidationError::invalid_argument("Payment method is required");
    if (amount_cents != state.total_cents) throw ValidationError::invalid_argument("Payment amount must match order total");

    examples::PaymentSubmitted event;
    event.set_payment_method(payment_method);
    event.set_payment_reference(payment_reference);
    event.set_amount_cents(amount_cents);
    event.mutable_submitted_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

examples::OrderCompleted OrderLogic::handle_complete_order(
    const OrderState& state, int32_t loyalty_points_earned) {
    if (!state.exists()) throw ValidationError::failed_precondition("Order does not exist");
    if (state.status != OrderStatus::PaymentPending) throw ValidationError::failed_precondition("Order not awaiting completion");

    examples::OrderCompleted event;
    event.set_loyalty_points_earned(loyalty_points_earned);
    event.mutable_completed_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

examples::OrderCancelled OrderLogic::handle_cancel_order(
    const OrderState& state, const std::string& reason) {
    if (!state.exists()) throw ValidationError::failed_precondition("Order does not exist");
    if (state.status == OrderStatus::Completed) throw ValidationError::failed_precondition("Cannot cancel completed order");
    if (state.status == OrderStatus::Cancelled) throw ValidationError::failed_precondition("Order already cancelled");

    examples::OrderCancelled event;
    event.set_reason(reason);
    event.set_loyalty_points_refunded(state.loyalty_points_used);
    event.mutable_cancelled_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

OrderState OrderLogic::apply_event(OrderState state, const google::protobuf::Any& event) {
    const auto& type_url = event.type_url();

    if (type_url.find("OrderCreated") != std::string::npos) {
        examples::OrderCreated e;
        event.UnpackTo(&e);
        state.customer_id = e.customer_id();
        state.items.clear();
        for (const auto& item : e.items()) {
            state.items.push_back({item.product_id(), item.quantity(), item.price_cents()});
        }
        state.subtotal_cents = e.subtotal_cents();
        state.discount_cents = e.discount_cents();
        state.total_cents = e.total_cents();
        state.status = OrderStatus::Created;
    } else if (type_url.find("LoyaltyDiscountApplied") != std::string::npos) {
        examples::LoyaltyDiscountApplied e;
        event.UnpackTo(&e);
        state.loyalty_points_used = e.points_used();
        state.discount_cents += e.discount_cents();
        state.total_cents -= e.discount_cents();
    } else if (type_url.find("PaymentSubmitted") != std::string::npos) {
        state.status = OrderStatus::PaymentPending;
    } else if (type_url.find("OrderCompleted") != std::string::npos) {
        examples::OrderCompleted e;
        event.UnpackTo(&e);
        state.loyalty_points_earned = e.loyalty_points_earned();
        state.status = OrderStatus::Completed;
    } else if (type_url.find("OrderCancelled") != std::string::npos) {
        state.status = OrderStatus::Cancelled;
    }

    return state;
}

}  // namespace order
