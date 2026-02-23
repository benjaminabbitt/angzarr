#pragma once

#include "examples/hand.pb.h"
#include "hand_state.hpp"

namespace hand {
namespace handlers {

/// Handle PostBlind command.
examples::BlindPosted handle_post_blind(const examples::PostBlind& cmd, const HandState& state);

}  // namespace handlers
}  // namespace hand
