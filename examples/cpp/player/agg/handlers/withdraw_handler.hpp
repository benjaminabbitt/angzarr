#pragma once

#include "examples/player.pb.h"
#include "player_state.hpp"

namespace player {
namespace handlers {

/// Handle WithdrawFunds command.
examples::FundsWithdrawn handle_withdraw(const examples::WithdrawFunds& cmd,
                                         const PlayerState& state);

}  // namespace handlers
}  // namespace player
