#include "join_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <google/protobuf/util/time_util.h>

namespace table {
namespace handlers {

examples::PlayerJoined handle_join(
    const examples::JoinTable& cmd,
    const TableState& state) {

    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::not_found("Table does not exist");
    }

    // Validate
    if (cmd.player_root().empty()) {
        throw angzarr::CommandRejectedError::invalid_argument("player_root is required");
    }
    if (state.find_player_seat(cmd.player_root())) {
        throw angzarr::CommandRejectedError::precondition_failed("Player already seated at table");
    }
    if (state.is_full()) {
        throw angzarr::CommandRejectedError::precondition_failed("Table is full");
    }
    if (cmd.buy_in_amount() < state.min_buy_in) {
        throw angzarr::CommandRejectedError::invalid_argument(
            "Buy-in must be at least " + std::to_string(state.min_buy_in));
    }
    if (cmd.buy_in_amount() > state.max_buy_in) {
        throw angzarr::CommandRejectedError::invalid_argument(
            "Buy-in cannot exceed " + std::to_string(state.max_buy_in));
    }
    // preferred_seat > 0 means explicit preference; 0 means no preference
    if (cmd.preferred_seat() > 0 && state.get_seat(cmd.preferred_seat()) != nullptr) {
        throw angzarr::CommandRejectedError::precondition_failed("Seat is occupied");
    }

    // Compute
    int seat_position = state.find_available_seat(cmd.preferred_seat());

    auto now = std::chrono::system_clock::now();
    auto timestamp = google::protobuf::util::TimeUtil::TimeTToTimestamp(
        std::chrono::system_clock::to_time_t(now));

    examples::PlayerJoined event;
    event.set_player_root(cmd.player_root());
    event.set_seat_position(seat_position);
    event.set_buy_in_amount(cmd.buy_in_amount());
    event.set_stack(cmd.buy_in_amount());
    *event.mutable_joined_at() = timestamp;

    return event;
}

} // namespace handlers
} // namespace table
