#pragma once

#include "hand_state.hpp"
#include "examples/hand.pb.h"

namespace hand {
namespace handlers {

/// Handle PlayerAction command.
examples::ActionTaken handle_action(
    const examples::PlayerAction& cmd,
    const HandState& state);

} // namespace handlers
} // namespace hand
