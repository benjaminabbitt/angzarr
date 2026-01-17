#include "fulfillment_logic.hpp"
#include "validation_error.hpp"
#include <chrono>

namespace fulfillment {

using namespace angzarr;

FulfillmentState FulfillmentLogic::rebuild_state(const angzarr::EventBook* event_book) {
    FulfillmentState state;
    if (!event_book || event_book->pages().empty()) return state;

    for (const auto& page : event_book->pages()) {
        if (page.has_event()) {
            state = apply_event(std::move(state), page.event());
        }
    }
    return state;
}

examples::ShipmentCreated FulfillmentLogic::handle_create_shipment(
    const FulfillmentState& state, const std::string& order_id,
    const std::vector<FulfillmentItem>& items) {
    if (state.exists()) throw ValidationError::failed_precondition("Shipment already exists");
    if (order_id.empty()) throw ValidationError::invalid_argument("Order ID is required");
    if (items.empty()) throw ValidationError::invalid_argument("Shipment must have items");

    examples::ShipmentCreated event;
    event.set_order_id(order_id);
    for (const auto& item : items) {
        auto* shipment_item = event.add_items();
        shipment_item->set_product_id(item.product_id);
        shipment_item->set_quantity(item.quantity);
    }
    event.mutable_created_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

examples::ItemsPicked FulfillmentLogic::handle_mark_picked(const FulfillmentState& state) {
    if (!state.exists()) throw ValidationError::failed_precondition("Shipment does not exist");
    if (state.status != FulfillmentStatus::Pending) throw ValidationError::failed_precondition("Shipment not in pending state");

    examples::ItemsPicked event;
    event.mutable_picked_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

examples::ItemsPacked FulfillmentLogic::handle_mark_packed(const FulfillmentState& state) {
    if (!state.exists()) throw ValidationError::failed_precondition("Shipment does not exist");
    if (state.status != FulfillmentStatus::Picking) throw ValidationError::failed_precondition("Items must be picked first");

    examples::ItemsPacked event;
    event.mutable_packed_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

examples::Shipped FulfillmentLogic::handle_ship(
    const FulfillmentState& state, const std::string& tracking_number,
    const std::string& carrier) {
    if (!state.exists()) throw ValidationError::failed_precondition("Shipment does not exist");
    if (state.status != FulfillmentStatus::Packing) throw ValidationError::failed_precondition("Items must be packed first");
    if (tracking_number.empty()) throw ValidationError::invalid_argument("Tracking number is required");
    if (carrier.empty()) throw ValidationError::invalid_argument("Carrier is required");

    examples::Shipped event;
    event.set_tracking_number(tracking_number);
    event.set_carrier(carrier);
    event.mutable_shipped_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

examples::Delivered FulfillmentLogic::handle_record_delivery(const FulfillmentState& state) {
    if (!state.exists()) throw ValidationError::failed_precondition("Shipment does not exist");
    if (state.status != FulfillmentStatus::Shipped) throw ValidationError::failed_precondition("Shipment must be shipped first");

    examples::Delivered event;
    event.mutable_delivered_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

FulfillmentState FulfillmentLogic::apply_event(FulfillmentState state, const google::protobuf::Any& event) {
    const auto& type_url = event.type_url();

    if (type_url.find("ShipmentCreated") != std::string::npos) {
        examples::ShipmentCreated e;
        event.UnpackTo(&e);
        state.order_id = e.order_id();
        state.items.clear();
        for (const auto& item : e.items()) {
            state.items.push_back({item.product_id(), item.quantity()});
        }
        state.status = FulfillmentStatus::Pending;
    } else if (type_url.find("ItemsPicked") != std::string::npos) {
        state.status = FulfillmentStatus::Picking;
    } else if (type_url.find("ItemsPacked") != std::string::npos) {
        state.status = FulfillmentStatus::Packing;
    } else if (type_url.find("Shipped") != std::string::npos) {
        examples::Shipped e;
        event.UnpackTo(&e);
        state.tracking_number = e.tracking_number();
        state.carrier = e.carrier();
        state.status = FulfillmentStatus::Shipped;
    } else if (type_url.find("Delivered") != std::string::npos) {
        state.status = FulfillmentStatus::Delivered;
    }

    return state;
}

}  // namespace fulfillment
