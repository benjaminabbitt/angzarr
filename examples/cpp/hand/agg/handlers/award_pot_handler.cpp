#include "award_pot_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <google/protobuf/util/time_util.h>

namespace hand {
namespace handlers {

std::pair<examples::PotAwarded, examples::HandComplete> handle_award_pot(
    const examples::AwardPot& cmd,
    const HandState& state) {

    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::not_found("Hand not dealt");
    }
    if (state.status == "complete") {
        throw angzarr::CommandRejectedError::precondition_failed("Hand already complete");
    }

    // Validate
    if (cmd.awards().empty()) {
        throw angzarr::CommandRejectedError::invalid_argument("No awards specified");
    }

    for (const auto& award : cmd.awards()) {
        const PlayerHandInfo* player = state.get_player(award.player_root());
        if (!player) {
            throw angzarr::CommandRejectedError::not_found("Winner not in hand");
        }
        if (player->has_folded) {
            throw angzarr::CommandRejectedError::precondition_failed(
                "Folded player cannot win pot");
        }
    }

    // Compute
    auto now = std::chrono::system_clock::now();
    auto timestamp = google::protobuf::util::TimeUtil::TimeTToTimestamp(
        std::chrono::system_clock::to_time_t(now));

    // Adjust awards to match pot if needed
    int64_t total_awarded = 0;
    for (const auto& award : cmd.awards()) {
        total_awarded += award.amount();
    }
    int64_t pot_total = state.get_pot_total();

    std::vector<examples::PotAward> adjusted_awards(cmd.awards().begin(), cmd.awards().end());
    if (total_awarded != pot_total && pot_total > 0 && !adjusted_awards.empty()) {
        int64_t others_sum = 0;
        for (size_t i = 1; i < adjusted_awards.size(); ++i) {
            others_sum += adjusted_awards[i].amount();
        }
        adjusted_awards[0].set_amount(pot_total - others_sum);
    }

    // Build PotAwarded event
    examples::PotAwarded pot_event;
    *pot_event.mutable_awarded_at() = timestamp;
    for (const auto& award : adjusted_awards) {
        auto* winner = pot_event.add_winners();
        winner->set_player_root(award.player_root());
        winner->set_amount(award.amount());
        winner->set_pot_type(award.pot_type());
    }

    // Build HandComplete event
    examples::HandComplete complete_event;
    complete_event.set_table_root(state.table_root);
    complete_event.set_hand_number(state.hand_number);
    *complete_event.mutable_completed_at() = timestamp;

    // Copy winners
    for (const auto& winner : pot_event.winners()) {
        *complete_event.add_winners() = winner;
    }

    // Build final stacks
    for (const auto& [pos, player] : state.players) {
        int64_t player_winnings = 0;
        for (const auto& award : adjusted_awards) {
            if (award.player_root() == player.player_root) {
                player_winnings += award.amount();
            }
        }

        auto* stack_snapshot = complete_event.add_final_stacks();
        stack_snapshot->set_player_root(player.player_root);
        stack_snapshot->set_stack(player.stack + player_winnings);
        stack_snapshot->set_is_all_in(player.is_all_in);
        stack_snapshot->set_has_folded(player.has_folded);
    }

    return {pot_event, complete_event};
}

} // namespace handlers
} // namespace hand
