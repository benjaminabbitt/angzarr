#include "inventory_logic.hpp"
#include "validation_error.hpp"
#include <chrono>

namespace inventory {

using namespace angzarr;

InventoryState InventoryLogic::rebuild_state(const angzarr::EventBook* event_book) {
    InventoryState state;
    if (!event_book || event_book->pages().empty()) return state;

    for (const auto& page : event_book->pages()) {
        if (page.has_event()) {
            state = apply_event(std::move(state), page.event());
        }
    }
    return state;
}

examples::StockInitialized InventoryLogic::handle_initialize_stock(
    const InventoryState& state, const std::string& product_id, int32_t quantity) {
    if (state.exists()) throw ValidationError::failed_precondition("Inventory already exists");
    if (product_id.empty()) throw ValidationError::invalid_argument("Product ID is required");
    if (quantity < 0) throw ValidationError::invalid_argument("Quantity cannot be negative");

    examples::StockInitialized event;
    event.set_product_id(product_id);
    event.set_initial_quantity(quantity);
    return event;
}

examples::StockReceived InventoryLogic::handle_receive_stock(
    const InventoryState& state, int32_t quantity, const std::string& reference) {
    if (!state.exists()) throw ValidationError::failed_precondition("Inventory does not exist");
    if (quantity <= 0) throw ValidationError::invalid_argument("Quantity must be positive");

    examples::StockReceived event;
    event.set_quantity(quantity);
    event.set_reference(reference);
    event.mutable_received_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

examples::StockReserved InventoryLogic::handle_reserve_stock(
    const InventoryState& state, const std::string& order_id, int32_t quantity) {
    if (!state.exists()) throw ValidationError::failed_precondition("Inventory does not exist");
    if (order_id.empty()) throw ValidationError::invalid_argument("Order ID is required");
    if (quantity <= 0) throw ValidationError::invalid_argument("Quantity must be positive");
    if (state.reservations.find(order_id) != state.reservations.end()) {
        throw ValidationError::failed_precondition("Reservation already exists for this order");
    }
    if (quantity > state.available()) throw ValidationError::failed_precondition("Insufficient stock");

    examples::StockReserved event;
    event.set_order_id(order_id);
    event.set_quantity(quantity);
    event.set_available_after(state.available() - quantity);
    return event;
}

examples::ReservationReleased InventoryLogic::handle_release_reservation(
    const InventoryState& state, const std::string& order_id) {
    if (!state.exists()) throw ValidationError::failed_precondition("Inventory does not exist");
    auto it = state.reservations.find(order_id);
    if (it == state.reservations.end()) {
        throw ValidationError::failed_precondition("No reservation found for this order");
    }

    examples::ReservationReleased event;
    event.set_order_id(order_id);
    event.set_quantity_released(it->second.quantity);
    return event;
}

examples::ReservationCommitted InventoryLogic::handle_commit_reservation(
    const InventoryState& state, const std::string& order_id) {
    if (!state.exists()) throw ValidationError::failed_precondition("Inventory does not exist");
    auto it = state.reservations.find(order_id);
    if (it == state.reservations.end()) {
        throw ValidationError::failed_precondition("No reservation found for this order");
    }

    examples::ReservationCommitted event;
    event.set_order_id(order_id);
    event.set_quantity_committed(it->second.quantity);
    return event;
}

InventoryState InventoryLogic::apply_event(InventoryState state, const google::protobuf::Any& event) {
    const auto& type_url = event.type_url();

    if (type_url.find("StockInitialized") != std::string::npos) {
        examples::StockInitialized e;
        event.UnpackTo(&e);
        state.product_id = e.product_id();
        state.on_hand = e.initial_quantity();
    } else if (type_url.find("StockReceived") != std::string::npos) {
        examples::StockReceived e;
        event.UnpackTo(&e);
        state.on_hand += e.quantity();
    } else if (type_url.find("StockReserved") != std::string::npos) {
        examples::StockReserved e;
        event.UnpackTo(&e);
        state.reserved += e.quantity();
        state.reservations[e.order_id()] = {e.order_id(), e.quantity()};
    } else if (type_url.find("ReservationReleased") != std::string::npos) {
        examples::ReservationReleased e;
        event.UnpackTo(&e);
        auto it = state.reservations.find(e.order_id());
        if (it != state.reservations.end()) {
            state.reserved -= it->second.quantity;
            state.reservations.erase(it);
        }
    } else if (type_url.find("ReservationCommitted") != std::string::npos) {
        examples::ReservationCommitted e;
        event.UnpackTo(&e);
        auto it = state.reservations.find(e.order_id());
        if (it != state.reservations.end()) {
            state.on_hand -= it->second.quantity;
            state.reserved -= it->second.quantity;
            state.reservations.erase(it);
        }
    }

    return state;
}

}  // namespace inventory
