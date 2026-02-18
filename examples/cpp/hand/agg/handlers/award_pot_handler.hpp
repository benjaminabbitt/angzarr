#pragma once

#include "hand_state.hpp"
#include "examples/hand.pb.h"
#include <utility>

namespace hand {
namespace handlers {

/// Handle AwardPot command. Returns both PotAwarded and HandComplete events.
std::pair<examples::PotAwarded, examples::HandComplete> handle_award_pot(
    const examples::AwardPot& cmd,
    const HandState& state);

} // namespace handlers
} // namespace hand
