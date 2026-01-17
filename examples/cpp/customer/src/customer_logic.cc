#include "customer_logic.hpp"
#include "validation_error.hpp"
#include <chrono>

namespace customer {

using namespace angzarr;

CustomerState CustomerLogic::rebuild_state(const angzarr::EventBook* event_book) {
    CustomerState state;

    if (!event_book || event_book->pages().empty()) {
        return state;
    }

    // Handle snapshot if present
    if (event_book->has_snapshot() && event_book->snapshot().has_state()) {
        const auto& snap = event_book->snapshot().state();
        if (snap.type_url().find("CustomerState") != std::string::npos) {
            examples::CustomerState snap_state;
            snap.UnpackTo(&snap_state);
            state.name = snap_state.name();
            state.email = snap_state.email();
            state.loyalty_points = snap_state.loyalty_points();
            state.lifetime_points = snap_state.lifetime_points();
        }
    }

    // Apply events
    for (const auto& page : event_book->pages()) {
        if (page.has_event()) {
            state = apply_event(std::move(state), page.event());
        }
    }

    return state;
}

examples::CustomerCreated CustomerLogic::handle_create_customer(
    const CustomerState& state, const std::string& name, const std::string& email) {
    if (state.exists()) {
        throw ValidationError::failed_precondition("Customer already exists");
    }
    if (name.empty()) {
        throw ValidationError::invalid_argument("Customer name is required");
    }
    if (email.empty()) {
        throw ValidationError::invalid_argument("Customer email is required");
    }

    examples::CustomerCreated event;
    event.set_name(name);
    event.set_email(email);
    event.mutable_created_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

examples::LoyaltyPointsAdded CustomerLogic::handle_add_loyalty_points(
    const CustomerState& state, int32_t points, const std::string& reason) {
    if (!state.exists()) {
        throw ValidationError::failed_precondition("Customer does not exist");
    }
    if (points <= 0) {
        throw ValidationError::invalid_argument("Points must be positive");
    }

    examples::LoyaltyPointsAdded event;
    event.set_points(points);
    event.set_new_balance(state.loyalty_points + points);
    event.set_reason(reason);
    return event;
}

examples::LoyaltyPointsRedeemed CustomerLogic::handle_redeem_loyalty_points(
    const CustomerState& state, int32_t points, const std::string& redemption_type) {
    if (!state.exists()) {
        throw ValidationError::failed_precondition("Customer does not exist");
    }
    if (points <= 0) {
        throw ValidationError::invalid_argument("Points must be positive");
    }
    if (points > state.loyalty_points) {
        throw ValidationError::failed_precondition(
            "Insufficient points: have " + std::to_string(state.loyalty_points) +
            ", need " + std::to_string(points));
    }

    examples::LoyaltyPointsRedeemed event;
    event.set_points(points);
    event.set_new_balance(state.loyalty_points - points);
    event.set_redemption_type(redemption_type);
    return event;
}

CustomerState CustomerLogic::apply_event(CustomerState state, const google::protobuf::Any& event) {
    const auto& type_url = event.type_url();

    if (type_url.find("CustomerCreated") != std::string::npos) {
        examples::CustomerCreated e;
        event.UnpackTo(&e);
        state.name = e.name();
        state.email = e.email();
    } else if (type_url.find("LoyaltyPointsAdded") != std::string::npos) {
        examples::LoyaltyPointsAdded e;
        event.UnpackTo(&e);
        state.loyalty_points = e.new_balance();
        state.lifetime_points += e.points();
    } else if (type_url.find("LoyaltyPointsRedeemed") != std::string::npos) {
        examples::LoyaltyPointsRedeemed e;
        event.UnpackTo(&e);
        state.loyalty_points = e.new_balance();
    }

    return state;
}

}  // namespace customer
