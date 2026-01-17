#pragma once

#include <string>
#include <vector>
#include "angzarr.pb.h"
#include "domains.pb.h"

namespace cart {

enum class CartStatus { Uninitialized, Active, CheckedOut };

struct CartItem {
    std::string product_id;
    int32_t quantity;
    int32_t price_cents;
};

struct CartState {
    std::string customer_id;
    std::vector<CartItem> items;
    std::string coupon_code;
    int32_t discount_cents = 0;
    CartStatus status = CartStatus::Uninitialized;

    bool exists() const { return status != CartStatus::Uninitialized; }
    bool active() const { return status == CartStatus::Active; }
    int32_t subtotal() const;
    int32_t find_item_index(const std::string& product_id) const;
};

class CartLogic {
public:
    static CartState rebuild_state(const angzarr::EventBook* event_book);

    static examples::CartCreated handle_create_cart(
        const CartState& state, const std::string& customer_id);

    static examples::ItemAdded handle_add_item(
        const CartState& state, const std::string& product_id,
        int32_t quantity, int32_t price_cents);

    static examples::QuantityUpdated handle_update_quantity(
        const CartState& state, const std::string& product_id, int32_t quantity);

    static examples::ItemRemoved handle_remove_item(
        const CartState& state, const std::string& product_id);

    static examples::CouponApplied handle_apply_coupon(
        const CartState& state, const std::string& coupon_code, int32_t discount_cents);

    static examples::CartCleared handle_clear_cart(const CartState& state);

private:
    static CartState apply_event(CartState state, const google::protobuf::Any& event);
};

}  // namespace cart
