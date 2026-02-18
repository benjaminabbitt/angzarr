#include "player.hpp"
#include "register_handler.hpp"
#include "deposit_handler.hpp"
#include "withdraw_handler.hpp"
#include "reserve_handler.hpp"
#include "release_handler.hpp"
#include "transfer_handler.hpp"

namespace player {

// docs:start:oo_handlers
Player::Player() {
    // Register command handlers
    register_handler<examples::RegisterPlayer, examples::PlayerRegistered>(
        [this](const examples::RegisterPlayer& cmd, const PlayerState&) {
            return handle_register(cmd);
        }
    );
    register_handler<examples::DepositFunds, examples::FundsDeposited>(
        [this](const examples::DepositFunds& cmd, const PlayerState&) {
            return handle_deposit(cmd);
        }
    );
    register_handler<examples::WithdrawFunds, examples::FundsWithdrawn>(
        [this](const examples::WithdrawFunds& cmd, const PlayerState&) {
            return handle_withdraw(cmd);
        }
    );
    register_handler<examples::ReserveFunds, examples::FundsReserved>(
        [this](const examples::ReserveFunds& cmd, const PlayerState&) {
            return handle_reserve(cmd);
        }
    );
    register_handler<examples::ReleaseFunds, examples::FundsReleased>(
        [this](const examples::ReleaseFunds& cmd, const PlayerState&) {
            return handle_release(cmd);
        }
    );
    register_handler<examples::TransferFunds, examples::FundsTransferred>(
        [this](const examples::TransferFunds& cmd, const PlayerState&) {
            return handle_transfer(cmd);
        }
    );
}

examples::PlayerRegistered Player::handle_register(const examples::RegisterPlayer& cmd) {
    return handlers::handle_register(cmd, state_);
}

examples::FundsDeposited Player::handle_deposit(const examples::DepositFunds& cmd) {
    return handlers::handle_deposit(cmd, state_);
}

examples::FundsWithdrawn Player::handle_withdraw(const examples::WithdrawFunds& cmd) {
    return handlers::handle_withdraw(cmd, state_);
}

examples::FundsReserved Player::handle_reserve(const examples::ReserveFunds& cmd) {
    return handlers::handle_reserve(cmd, state_);
}

examples::FundsReleased Player::handle_release(const examples::ReleaseFunds& cmd) {
    return handlers::handle_release(cmd, state_);
}

examples::FundsTransferred Player::handle_transfer(const examples::TransferFunds& cmd) {
    return handlers::handle_transfer(cmd, state_);
}
// docs:end:oo_handlers

} // namespace player
