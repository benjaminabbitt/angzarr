#include "product_logic.hpp"
#include "validation_error.hpp"
#include <chrono>

namespace product {

using namespace angzarr;

ProductState ProductLogic::rebuild_state(const angzarr::EventBook* event_book) {
    ProductState state;
    if (!event_book || event_book->pages().empty()) return state;

    for (const auto& page : event_book->pages()) {
        if (page.has_event()) {
            state = apply_event(std::move(state), page.event());
        }
    }
    return state;
}

examples::ProductCreated ProductLogic::handle_create_product(
    const ProductState& state, const std::string& sku, const std::string& name,
    const std::string& description, int32_t price_cents) {
    if (state.exists()) throw ValidationError::failed_precondition("Product already exists");
    if (sku.empty()) throw ValidationError::invalid_argument("SKU is required");
    if (name.empty()) throw ValidationError::invalid_argument("Name is required");
    if (price_cents <= 0) throw ValidationError::invalid_argument("Price must be positive");

    examples::ProductCreated event;
    event.set_sku(sku);
    event.set_name(name);
    event.set_description(description);
    event.set_price_cents(price_cents);
    event.mutable_created_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

examples::ProductUpdated ProductLogic::handle_update_product(
    const ProductState& state, const std::string& name, const std::string& description) {
    if (!state.exists()) throw ValidationError::failed_precondition("Product does not exist");
    if (!state.active()) throw ValidationError::failed_precondition("Product is discontinued");

    examples::ProductUpdated event;
    event.set_name(name.empty() ? state.name : name);
    event.set_description(description.empty() ? state.description : description);
    return event;
}

examples::PriceSet ProductLogic::handle_set_price(const ProductState& state, int32_t price_cents) {
    if (!state.exists()) throw ValidationError::failed_precondition("Product does not exist");
    if (!state.active()) throw ValidationError::failed_precondition("Product is discontinued");
    if (price_cents <= 0) throw ValidationError::invalid_argument("Price must be positive");

    examples::PriceSet event;
    event.set_old_price_cents(state.price_cents);
    event.set_new_price_cents(price_cents);
    return event;
}

examples::ProductDiscontinued ProductLogic::handle_discontinue(
    const ProductState& state, const std::string& reason) {
    if (!state.exists()) throw ValidationError::failed_precondition("Product does not exist");
    if (!state.active()) throw ValidationError::failed_precondition("Product already discontinued");

    examples::ProductDiscontinued event;
    event.set_reason(reason);
    event.mutable_discontinued_at()->set_seconds(
        std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));
    return event;
}

ProductState ProductLogic::apply_event(ProductState state, const google::protobuf::Any& event) {
    const auto& type_url = event.type_url();

    if (type_url.find("ProductCreated") != std::string::npos) {
        examples::ProductCreated e;
        event.UnpackTo(&e);
        state.sku = e.sku();
        state.name = e.name();
        state.description = e.description();
        state.price_cents = e.price_cents();
        state.status = ProductStatus::Active;
    } else if (type_url.find("ProductUpdated") != std::string::npos) {
        examples::ProductUpdated e;
        event.UnpackTo(&e);
        state.name = e.name();
        state.description = e.description();
    } else if (type_url.find("PriceSet") != std::string::npos) {
        examples::PriceSet e;
        event.UnpackTo(&e);
        state.price_cents = e.new_price_cents();
    } else if (type_url.find("ProductDiscontinued") != std::string::npos) {
        state.status = ProductStatus::Discontinued;
    }

    return state;
}

}  // namespace product
