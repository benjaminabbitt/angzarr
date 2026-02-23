#pragma once

#include "examples/hand.pb.h"
#include "hand_state.hpp"

namespace hand {
namespace handlers {

/// Handle DealCommunityCards command.
examples::CommunityCardsDealt handle_deal_community(const examples::DealCommunityCards& cmd,
                                                    const HandState& state);

}  // namespace handlers
}  // namespace hand
