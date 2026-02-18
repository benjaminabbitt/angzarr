#include "transfer_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>

namespace player {
namespace handlers {

examples::FundsTransferred handle_transfer(const examples::TransferFunds& cmd, const PlayerState& state) {
    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::precondition_failed("Player does not exist");
    }

    // Compute
    int64_t amount = cmd.has_amount() ? cmd.amount().amount() : 0;
    int64_t new_balance = state.bankroll + amount;

    examples::FundsTransferred event;
    event.set_from_player_root(cmd.from_player_root());
    event.set_to_player_root(state.player_id);
    event.mutable_amount()->CopyFrom(cmd.amount());
    event.set_hand_root(cmd.hand_root());
    event.set_reason(cmd.reason());
    event.mutable_new_balance()->set_amount(new_balance);
    event.mutable_new_balance()->set_currency_code("CHIPS");

    auto now = std::chrono::system_clock::now();
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch()).count();
    event.mutable_transferred_at()->set_seconds(seconds);

    return event;
}

} // namespace handlers
} // namespace player
