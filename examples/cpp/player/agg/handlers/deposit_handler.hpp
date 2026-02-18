#pragma once

#include "player_state.hpp"
#include "examples/player.pb.h"

namespace player {
namespace handlers {

/// Handle DepositFunds command.
examples::FundsDeposited handle_deposit(const examples::DepositFunds& cmd, const PlayerState& state);

} // namespace handlers
} // namespace player
