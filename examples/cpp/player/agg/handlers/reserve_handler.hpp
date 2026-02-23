#pragma once

#include "examples/player.pb.h"
#include "player_state.hpp"

namespace player {
namespace handlers {

/// Handle ReserveFunds command.
examples::FundsReserved handle_reserve(const examples::ReserveFunds& cmd, const PlayerState& state);

}  // namespace handlers
}  // namespace player
