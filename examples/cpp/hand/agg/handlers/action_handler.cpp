#include "action_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <google/protobuf/util/time_util.h>

namespace hand {
namespace handlers {

examples::ActionTaken handle_action(
    const examples::PlayerAction& cmd,
    const HandState& state) {

    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::not_found("Hand not dealt");
    }
    if (state.status != "betting") {
        throw angzarr::CommandRejectedError::precondition_failed("Not in betting phase");
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
    if (player->is_all_in) {
        throw angzarr::CommandRejectedError::precondition_failed("Player is all-in");
    }

    // Compute
    examples::ActionType action = cmd.action();
    int64_t amount = cmd.amount();
    int64_t call_amount = state.current_bet - player->bet_this_round;

    if (action == examples::FOLD) {
        amount = 0;
    } else if (action == examples::CHECK) {
        if (call_amount > 0) {
            throw angzarr::CommandRejectedError::precondition_failed(
                "Cannot check when there is a bet to call");
        }
        amount = 0;
    } else if (action == examples::CALL) {
        if (call_amount == 0) {
            throw angzarr::CommandRejectedError::precondition_failed("Nothing to call");
        }
        int64_t actual_amount = std::min(call_amount, player->stack);
        amount = actual_amount;
        if (player->stack - actual_amount == 0) {
            action = examples::ALL_IN;
        }
    } else if (action == examples::BET) {
        if (state.current_bet > 0) {
            throw angzarr::CommandRejectedError::precondition_failed(
                "Cannot bet when there is already a bet");
        }
        if (amount < state.big_blind) {
            throw angzarr::CommandRejectedError::invalid_argument(
                "Bet must be at least " + std::to_string(state.big_blind));
        }
        if (amount > player->stack) {
            throw angzarr::CommandRejectedError::invalid_argument("Bet exceeds stack");
        }
        if (player->stack - amount == 0) {
            action = examples::ALL_IN;
        }
    } else if (action == examples::RAISE) {
        if (state.current_bet == 0) {
            throw angzarr::CommandRejectedError::precondition_failed(
                "Cannot raise when there is no bet");
        }
        int64_t total_bet = player->bet_this_round + amount;
        int64_t raise_amount = total_bet - state.current_bet;
        if (raise_amount < state.min_raise && amount < player->stack) {
            throw angzarr::CommandRejectedError::invalid_argument(
                "Raise must be at least " + std::to_string(state.min_raise));
        }
        if (amount > player->stack) {
            throw angzarr::CommandRejectedError::invalid_argument("Raise exceeds stack");
        }
        if (player->stack - amount == 0) {
            action = examples::ALL_IN;
        }
    } else if (action == examples::ALL_IN) {
        amount = player->stack;
    } else {
        throw angzarr::CommandRejectedError::invalid_argument("Invalid action");
    }

    int64_t new_stack = player->stack - amount;
    int64_t new_pot_total = state.get_pot_total() + amount;
    int64_t new_bet = player->bet_this_round + amount;
    int64_t amount_to_call = std::max(state.current_bet, new_bet) - player->bet_this_round;

    auto now = std::chrono::system_clock::now();
    auto timestamp = google::protobuf::util::TimeUtil::TimeTToTimestamp(
        std::chrono::system_clock::to_time_t(now));

    examples::ActionTaken event;
    event.set_player_root(cmd.player_root());
    event.set_action(action);
    event.set_amount(amount);
    event.set_player_stack(new_stack);
    event.set_pot_total(new_pot_total);
    event.set_amount_to_call(amount_to_call);
    *event.mutable_action_at() = timestamp;

    return event;
}

} // namespace handlers
} // namespace hand
