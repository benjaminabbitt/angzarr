#pragma once

#include <string>
#include <unordered_map>
#include "angzarr.pb.h"
#include "domains.pb.h"

namespace inventory {

struct Reservation {
    std::string order_id;
    int32_t quantity;
};

struct InventoryState {
    std::string product_id;
    int32_t on_hand = 0;
    int32_t reserved = 0;
    std::unordered_map<std::string, Reservation> reservations;

    bool exists() const { return !product_id.empty(); }
    int32_t available() const { return on_hand - reserved; }
};

class InventoryLogic {
public:
    static InventoryState rebuild_state(const angzarr::EventBook* event_book);

    static examples::StockInitialized handle_initialize_stock(
        const InventoryState& state, const std::string& product_id, int32_t quantity);

    static examples::StockReceived handle_receive_stock(
        const InventoryState& state, int32_t quantity, const std::string& reference);

    static examples::StockReserved handle_reserve_stock(
        const InventoryState& state, const std::string& order_id, int32_t quantity);

    static examples::ReservationReleased handle_release_reservation(
        const InventoryState& state, const std::string& order_id);

    static examples::ReservationCommitted handle_commit_reservation(
        const InventoryState& state, const std::string& order_id);

private:
    static InventoryState apply_event(InventoryState state, const google::protobuf::Any& event);
};

}  // namespace inventory
