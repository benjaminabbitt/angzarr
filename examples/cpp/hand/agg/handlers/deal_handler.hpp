#pragma once

#include "hand_state.hpp"
#include "examples/hand.pb.h"

namespace hand {
namespace handlers {

/// Handle DealCards command.
examples::CardsDealt handle_deal(
    const examples::DealCards& cmd,
    const HandState& state);

} // namespace handlers
} // namespace hand
