#include "create_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <google/protobuf/util/time_util.h>

namespace table {
namespace handlers {

examples::TableCreated handle_create(
    const examples::CreateTable& cmd,
    const TableState& state) {

    // Guard
    if (state.exists()) {
        throw angzarr::CommandRejectedError::precondition_failed("Table already exists");
    }

    // Validate
    if (cmd.table_name().empty()) {
        throw angzarr::CommandRejectedError::invalid_argument("table_name is required");
    }
    if (cmd.small_blind() <= 0) {
        throw angzarr::CommandRejectedError::invalid_argument("small_blind must be positive");
    }
    if (cmd.big_blind() <= 0) {
        throw angzarr::CommandRejectedError::invalid_argument("big_blind must be positive");
    }
    if (cmd.big_blind() < cmd.small_blind()) {
        throw angzarr::CommandRejectedError::invalid_argument("big_blind must be >= small_blind");
    }
    if (cmd.max_players() < 2 || cmd.max_players() > 10) {
        throw angzarr::CommandRejectedError::invalid_argument("max_players must be between 2 and 10");
    }

    // Compute
    auto now = std::chrono::system_clock::now();
    auto timestamp = google::protobuf::util::TimeUtil::TimeTToTimestamp(
        std::chrono::system_clock::to_time_t(now));

    examples::TableCreated event;
    event.set_table_name(cmd.table_name());
    event.set_game_variant(cmd.game_variant());
    event.set_small_blind(cmd.small_blind());
    event.set_big_blind(cmd.big_blind());
    event.set_min_buy_in(cmd.min_buy_in() > 0 ? cmd.min_buy_in() : cmd.big_blind() * 20);
    event.set_max_buy_in(cmd.max_buy_in() > 0 ? cmd.max_buy_in() : cmd.big_blind() * 100);
    event.set_max_players(cmd.max_players() > 0 ? cmd.max_players() : 9);
    event.set_action_timeout_seconds(cmd.action_timeout_seconds() > 0 ? cmd.action_timeout_seconds() : 30);
    *event.mutable_created_at() = timestamp;

    return event;
}

} // namespace handlers
} // namespace table
