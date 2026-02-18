#include "post_blind_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <google/protobuf/util/time_util.h>

namespace hand {
namespace handlers {

examples::BlindPosted handle_post_blind(
    const examples::PostBlind& cmd,
    const HandState& state) {

    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::not_found("Hand not dealt");
    }
    if (state.status == "complete") {
        throw angzarr::CommandRejectedError::precondition_failed("Hand is complete");
    }

    // Validate
    if (cmd.player_root().empty()) {
        throw angzarr::CommandRejectedError::invalid_argument("player_root is required");
    }

    const PlayerHandInfo* player = state.get_player(cmd.player_root());
    if (!player) {
        throw angzarr::CommandRejectedError::not_found("Player not in hand");
    }
    if (player->has_folded) {
        throw angzarr::CommandRejectedError::precondition_failed("Player has folded");
    }
    if (cmd.amount() <= 0) {
        throw angzarr::CommandRejectedError::invalid_argument("Blind amount must be positive");
    }

    // Compute
    int64_t actual_amount = std::min(cmd.amount(), player->stack);
    int64_t new_stack = player->stack - actual_amount;
    int64_t new_pot_total = state.get_pot_total() + actual_amount;

    auto now = std::chrono::system_clock::now();
    auto timestamp = google::protobuf::util::TimeUtil::TimeTToTimestamp(
        std::chrono::system_clock::to_time_t(now));

    examples::BlindPosted event;
    event.set_player_root(cmd.player_root());
    event.set_blind_type(cmd.blind_type());
    event.set_amount(actual_amount);
    event.set_player_stack(new_stack);
    event.set_pot_total(new_pot_total);
    *event.mutable_posted_at() = timestamp;

    return event;
}

} // namespace handlers
} // namespace hand
