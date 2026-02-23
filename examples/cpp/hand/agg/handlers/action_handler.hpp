#pragma once

#include "examples/hand.pb.h"
#include "hand_state.hpp"

namespace hand {
namespace handlers {

/// Handle PlayerAction command.
examples::ActionTaken handle_action(const examples::PlayerAction& cmd, const HandState& state);

}  // namespace handlers
}  // namespace hand
