#include "leave_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <google/protobuf/util/time_util.h>

namespace table {
namespace handlers {

examples::PlayerLeft handle_leave(
    const examples::LeaveTable& cmd,
    const TableState& state) {

    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::not_found("Table does not exist");
    }

    // Validate
    if (cmd.player_root().empty()) {
        throw angzarr::CommandRejectedError::invalid_argument("player_root is required");
    }

    const SeatState* seat = state.find_player_seat(cmd.player_root());
    if (!seat) {
        throw angzarr::CommandRejectedError::not_found("Player is not seated at table");
    }
    if (state.status == "in_hand") {
        throw angzarr::CommandRejectedError::precondition_failed("Cannot leave table during a hand");
    }

    // Compute
    auto now = std::chrono::system_clock::now();
    auto timestamp = google::protobuf::util::TimeUtil::TimeTToTimestamp(
        std::chrono::system_clock::to_time_t(now));

    examples::PlayerLeft event;
    event.set_player_root(cmd.player_root());
    event.set_seat_position(seat->position);
    event.set_chips_cashed_out(seat->stack);
    *event.mutable_left_at() = timestamp;

    return event;
}

} // namespace handlers
} // namespace table
