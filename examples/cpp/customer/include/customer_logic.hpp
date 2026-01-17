#pragma once

#include <string>
#include <optional>
#include "angzarr.pb.h"
#include "domains.pb.h"

namespace customer {

struct CustomerState {
    std::string name;
    std::string email;
    int32_t loyalty_points = 0;
    int32_t lifetime_points = 0;

    bool exists() const { return !name.empty(); }
};

class CustomerLogic {
public:
    static CustomerState rebuild_state(const angzarr::EventBook* event_book);

    static examples::CustomerCreated handle_create_customer(
        const CustomerState& state, const std::string& name, const std::string& email);

    static examples::LoyaltyPointsAdded handle_add_loyalty_points(
        const CustomerState& state, int32_t points, const std::string& reason);

    static examples::LoyaltyPointsRedeemed handle_redeem_loyalty_points(
        const CustomerState& state, int32_t points, const std::string& redemption_type);

private:
    static CustomerState apply_event(CustomerState state, const google::protobuf::Any& event);
};

}  // namespace customer
