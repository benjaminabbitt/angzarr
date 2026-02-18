#include "withdraw_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>

namespace player {
namespace handlers {

examples::FundsWithdrawn handle_withdraw(const examples::WithdrawFunds& cmd, const PlayerState& state) {
    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::precondition_failed("Player does not exist");
    }

    // Validate
    int64_t amount = cmd.has_amount() ? cmd.amount().amount() : 0;
    if (amount <= 0) {
        throw angzarr::CommandRejectedError::invalid_argument("amount must be positive");
    }
    if (amount > state.available_balance()) {
        throw angzarr::CommandRejectedError::precondition_failed("Insufficient funds");
    }

    // Compute
    int64_t new_balance = state.bankroll - amount;

    examples::FundsWithdrawn event;
    event.mutable_amount()->CopyFrom(cmd.amount());
    event.mutable_new_balance()->set_amount(new_balance);
    event.mutable_new_balance()->set_currency_code("CHIPS");

    auto now = std::chrono::system_clock::now();
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch()).count();
    event.mutable_withdrawn_at()->set_seconds(seconds);

    return event;
}

} // namespace handlers
} // namespace player
