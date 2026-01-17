#pragma once

#include <string>
#include <vector>
#include "angzarr.pb.h"
#include "domains.pb.h"

namespace fulfillment {

enum class FulfillmentStatus { Uninitialized, Pending, Picking, Packing, Shipped, Delivered };

struct FulfillmentItem {
    std::string product_id;
    int32_t quantity;
};

struct FulfillmentState {
    std::string order_id;
    std::vector<FulfillmentItem> items;
    std::string tracking_number;
    std::string carrier;
    FulfillmentStatus status = FulfillmentStatus::Uninitialized;

    bool exists() const { return status != FulfillmentStatus::Uninitialized; }
};

class FulfillmentLogic {
public:
    static FulfillmentState rebuild_state(const angzarr::EventBook* event_book);

    static examples::ShipmentCreated handle_create_shipment(
        const FulfillmentState& state, const std::string& order_id,
        const std::vector<FulfillmentItem>& items);

    static examples::ItemsPicked handle_mark_picked(const FulfillmentState& state);

    static examples::ItemsPacked handle_mark_packed(const FulfillmentState& state);

    static examples::Shipped handle_ship(
        const FulfillmentState& state, const std::string& tracking_number,
        const std::string& carrier);

    static examples::Delivered handle_record_delivery(const FulfillmentState& state);

private:
    static FulfillmentState apply_event(FulfillmentState state, const google::protobuf::Any& event);
};

}  // namespace fulfillment
