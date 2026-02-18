#pragma once

#include "player_state.hpp"
#include "examples/player.pb.h"

namespace player {
namespace handlers {

/// Handle TransferFunds command.
examples::FundsTransferred handle_transfer(const examples::TransferFunds& cmd, const PlayerState& state);

} // namespace handlers
} // namespace player
