#pragma once

#include "angzarr/router.hpp"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/player.pb.h"

namespace hand {
namespace saga {

/// Create the hand-player saga event router.
/// Reacts to PotAwarded events from Hand domain.
/// Sends DepositFunds commands to Player domain.
angzarr::EventRouter create_hand_player_router();

// Note: Handlers are internal to the router - only create_hand_player_router is public

}  // namespace saga
}  // namespace hand
