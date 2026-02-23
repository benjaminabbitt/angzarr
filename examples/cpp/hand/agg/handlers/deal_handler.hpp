#pragma once

#include "examples/hand.pb.h"
#include "hand_state.hpp"

namespace hand {
namespace handlers {

/// Handle DealCards command.
examples::CardsDealt handle_deal(const examples::DealCards& cmd, const HandState& state);

}  // namespace handlers
}  // namespace hand
