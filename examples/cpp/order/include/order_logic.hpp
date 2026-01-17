#pragma once

#include <string>
#include <vector>
#include "angzarr.pb.h"
#include "domains.pb.h"

namespace order {

enum class OrderStatus { Uninitialized, Created, PaymentPending, Completed, Cancelled };

struct OrderItem {
    std::string product_id;
    int32_t quantity;
    int32_t price_cents;
};

struct OrderState {
    std::string customer_id;
    std::vector<OrderItem> items;
    int32_t subtotal_cents = 0;
    int32_t discount_cents = 0;
    int32_t total_cents = 0;
    int32_t loyalty_points_used = 0;
    int32_t loyalty_points_earned = 0;
    OrderStatus status = OrderStatus::Uninitialized;

    bool exists() const { return status != OrderStatus::Uninitialized; }
    bool can_accept_payment() const { return status == OrderStatus::Created || status == OrderStatus::PaymentPending; }
};

class OrderLogic {
public:
    static OrderState rebuild_state(const angzarr::EventBook* event_book);

    static examples::OrderCreated handle_create_order(
        const OrderState& state, const std::string& customer_id,
        const std::vector<OrderItem>& items, int32_t subtotal_cents,
        int32_t discount_cents, int32_t total_cents);

    static examples::LoyaltyDiscountApplied handle_apply_loyalty_discount(
        const OrderState& state, int32_t points_used, int32_t discount_cents);

    static examples::PaymentSubmitted handle_submit_payment(
        const OrderState& state, const std::string& payment_method,
        const std::string& payment_reference, int32_t amount_cents);

    static examples::OrderCompleted handle_complete_order(
        const OrderState& state, int32_t loyalty_points_earned);

    static examples::OrderCancelled handle_cancel_order(
        const OrderState& state, const std::string& reason);

private:
    static OrderState apply_event(OrderState state, const google::protobuf::Any& event);
};

}  // namespace order
