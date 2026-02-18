#include "deposit_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>

namespace player {
namespace handlers {

// docs:start:deposit_guard
void guard(const PlayerState& state) {
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::precondition_failed("Player does not exist");
    }
}
// docs:end:deposit_guard

// docs:start:deposit_validate
int64_t validate(const examples::DepositFunds& cmd) {
    int64_t amount = cmd.has_amount() ? cmd.amount().amount() : 0;
    if (amount <= 0) {
        throw angzarr::CommandRejectedError::invalid_argument("amount must be positive");
    }
    return amount;
}
// docs:end:deposit_validate

// docs:start:deposit_compute
examples::FundsDeposited compute(const examples::DepositFunds& cmd, const PlayerState& state, int64_t amount) {
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
// docs:end:deposit_compute

examples::FundsDeposited handle_deposit(const examples::DepositFunds& cmd, const PlayerState& state) {
    guard(state);
    int64_t amount = validate(cmd);
    return compute(cmd, state, amount);
}

} // namespace handlers
} // namespace player
