#pragma once

#include "examples/player.pb.h"
#include "player_state.hpp"

namespace player {
namespace handlers {

/// Handle RegisterPlayer command.
examples::PlayerRegistered handle_register(const examples::RegisterPlayer& cmd,
                                           const PlayerState& state);

}  // namespace handlers
}  // namespace player
