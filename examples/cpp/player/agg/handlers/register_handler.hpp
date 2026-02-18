#pragma once

#include "player_state.hpp"
#include "examples/player.pb.h"

namespace player {
namespace handlers {

/// Handle RegisterPlayer command.
examples::PlayerRegistered handle_register(const examples::RegisterPlayer& cmd, const PlayerState& state);

} // namespace handlers
} // namespace player
