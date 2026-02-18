#pragma once

#include "hand_state.hpp"
#include "examples/hand.pb.h"

namespace hand {
namespace handlers {

/// Handle DealCommunityCards command.
examples::CommunityCardsDealt handle_deal_community(
    const examples::DealCommunityCards& cmd,
    const HandState& state);

} // namespace handlers
} // namespace hand
