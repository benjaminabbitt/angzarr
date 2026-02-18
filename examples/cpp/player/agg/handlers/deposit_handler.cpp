#include "deposit_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>

namespace player {
namespace handlers {

examples::FundsDeposited handle_deposit(const examples::DepositFunds& cmd, const PlayerState& state) {
    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::precondition_failed("Player does not exist");
    }

    // Validate
    int64_t amount = cmd.has_amount() ? cmd.amount().amount() : 0;
    if (amount <= 0) {
        throw angzarr::CommandRejectedError::invalid_argument("amount must be positive");
    }

    // Compute
    int64_t new_balance = state.bankroll + amount;

    examples::FundsDeposited event;
    event.mutable_amount()->CopyFrom(cmd.amount());
    event.mutable_new_balance()->set_amount(new_balance);
    event.mutable_new_balance()->set_currency_code("CHIPS");

    auto now = std::chrono::system_clock::now();
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch()).count();
    event.mutable_deposited_at()->set_seconds(seconds);

    return event;
}

} // namespace handlers
} // namespace player
