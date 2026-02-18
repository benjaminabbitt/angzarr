#pragma once

#include "hand_state.hpp"
#include "examples/hand.pb.h"

namespace hand {
namespace handlers {

/// Handle PostBlind command.
examples::BlindPosted handle_post_blind(
    const examples::PostBlind& cmd,
    const HandState& state);

} // namespace handlers
} // namespace hand
