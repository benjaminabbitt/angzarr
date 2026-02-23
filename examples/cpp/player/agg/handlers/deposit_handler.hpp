#pragma once

#include "examples/player.pb.h"
#include "player_state.hpp"

namespace player {
namespace handlers {

/// Handle DepositFunds command.
examples::FundsDeposited handle_deposit(const examples::DepositFunds& cmd,
                                        const PlayerState& state);

}  // namespace handlers
}  // namespace player
