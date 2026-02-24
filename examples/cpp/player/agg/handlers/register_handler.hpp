#pragma once

#include "examples/player.pb.h"
#include "player_state.hpp"

namespace player {
namespace handlers {

/// Handle RegisterPlayer command.
///
/// Follows the guard/validate/compute pattern:
/// - Guard: Check state preconditions (aggregate exists, correct phase)
/// - Validate: Validate command inputs
/// - Compute: Build the resulting event
///
/// Why this pattern? Each step is a pure function (state in, result out),
/// enabling direct unit testing without mocking infrastructure.
examples::PlayerRegistered handle_register(const examples::RegisterPlayer& cmd,
                                           const PlayerState& state);

}  // namespace handlers
}  // namespace player
