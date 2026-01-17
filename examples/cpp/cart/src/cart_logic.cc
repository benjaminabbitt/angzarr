#include "cart_logic.hpp"
#include "validation_error.hpp"
#include <algorithm>

namespace cart {

using namespace angzarr;

int32_t CartState::subtotal() const {
    int32_t total = 0;
    for (const auto& item : items) {
        total += item.quantity * item.price_cents;
    }
    return total;
}

int32_t CartState::find_item_index(const std::string& product_id) const {
    for (size_t i = 0; i < items.size(); ++i) {
        if (items[i].product_id == product_id) return static_cast<int32_t>(i);
    }
    return -1;
}

CartState CartLogic::rebuild_state(const angzarr::EventBook* event_book) {
    CartState state;
    if (!event_book || event_book->pages().empty()) return state;

    for (const auto& page : event_book->pages()) {
        if (page.has_event()) {
            state = apply_event(std::move(state), page.event());
        }
    }
    return state;
}

examples::CartCreated CartLogic::handle_create_cart(
    const CartState& state, const std::string& customer_id) {
    if (state.exists()) throw ValidationError::failed_precondition("Cart already exists");
    if (customer_id.empty()) throw ValidationError::invalid_argument("Customer ID is required");

    examples::CartCreated event;
    event.set_customer_id(customer_id);
    return event;
}

examples::ItemAdded CartLogic::handle_add_item(
    const CartState& state, const std::string& product_id,
    int32_t quantity, int32_t price_cents) {
    if (!state.exists()) throw ValidationError::failed_precondition("Cart does not exist");
    if (!state.active()) throw ValidationError::failed_precondition("Cart is not active");
    if (product_id.empty()) throw ValidationError::invalid_argument("Product ID is required");
    if (quantity <= 0) throw ValidationError::invalid_argument("Quantity must be positive");
    if (price_cents <= 0) throw ValidationError::invalid_argument("Price must be positive");

    examples::ItemAdded event;
    event.set_product_id(product_id);
    event.set_quantity(quantity);
    event.set_price_cents(price_cents);
    return event;
}

examples::QuantityUpdated CartLogic::handle_update_quantity(
    const CartState& state, const std::string& product_id, int32_t quantity) {
    if (!state.exists()) throw ValidationError::failed_precondition("Cart does not exist");
    if (!state.active()) throw ValidationError::failed_precondition("Cart is not active");
    if (state.find_item_index(product_id) < 0) throw ValidationError::failed_precondition("Item not in cart");
    if (quantity <= 0) throw ValidationError::invalid_argument("Quantity must be positive");

    examples::QuantityUpdated event;
    event.set_product_id(product_id);
    event.set_new_quantity(quantity);
    return event;
}

examples::ItemRemoved CartLogic::handle_remove_item(
    const CartState& state, const std::string& product_id) {
    if (!state.exists()) throw ValidationError::failed_precondition("Cart does not exist");
    if (!state.active()) throw ValidationError::failed_precondition("Cart is not active");
    if (state.find_item_index(product_id) < 0) throw ValidationError::failed_precondition("Item not in cart");

    examples::ItemRemoved event;
    event.set_product_id(product_id);
    return event;
}

examples::CouponApplied CartLogic::handle_apply_coupon(
    const CartState& state, const std::string& coupon_code, int32_t discount_cents) {
    if (!state.exists()) throw ValidationError::failed_precondition("Cart does not exist");
    if (!state.active()) throw ValidationError::failed_precondition("Cart is not active");
    if (coupon_code.empty()) throw ValidationError::invalid_argument("Coupon code is required");
    if (!state.coupon_code.empty()) throw ValidationError::failed_precondition("Coupon already applied");

    examples::CouponApplied event;
    event.set_coupon_code(coupon_code);
    event.set_discount_cents(discount_cents);
    return event;
}

examples::CartCleared CartLogic::handle_clear_cart(const CartState& state) {
    if (!state.exists()) throw ValidationError::failed_precondition("Cart does not exist");
    if (!state.active()) throw ValidationError::failed_precondition("Cart is not active");

    examples::CartCleared event;
    return event;
}

CartState CartLogic::apply_event(CartState state, const google::protobuf::Any& event) {
    const auto& type_url = event.type_url();

    if (type_url.find("CartCreated") != std::string::npos) {
        examples::CartCreated e;
        event.UnpackTo(&e);
        state.customer_id = e.customer_id();
        state.status = CartStatus::Active;
    } else if (type_url.find("ItemAdded") != std::string::npos) {
        examples::ItemAdded e;
        event.UnpackTo(&e);
        int32_t idx = state.find_item_index(e.product_id());
        if (idx >= 0) {
            state.items[idx].quantity += e.quantity();
        } else {
            state.items.push_back({e.product_id(), e.quantity(), e.price_cents()});
        }
    } else if (type_url.find("QuantityUpdated") != std::string::npos) {
        examples::QuantityUpdated e;
        event.UnpackTo(&e);
        int32_t idx = state.find_item_index(e.product_id());
        if (idx >= 0) state.items[idx].quantity = e.new_quantity();
    } else if (type_url.find("ItemRemoved") != std::string::npos) {
        examples::ItemRemoved e;
        event.UnpackTo(&e);
        int32_t idx = state.find_item_index(e.product_id());
        if (idx >= 0) state.items.erase(state.items.begin() + idx);
    } else if (type_url.find("CouponApplied") != std::string::npos) {
        examples::CouponApplied e;
        event.UnpackTo(&e);
        state.coupon_code = e.coupon_code();
        state.discount_cents = e.discount_cents();
    } else if (type_url.find("CartCleared") != std::string::npos) {
        state.items.clear();
        state.coupon_code.clear();
        state.discount_cents = 0;
    } else if (type_url.find("CartCheckoutCompleted") != std::string::npos) {
        state.status = CartStatus::CheckedOut;
    }

    return state;
}

}  // namespace cart
