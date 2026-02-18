#pragma once

#include "angzarr/event_router.hpp"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/player.pb.h"

namespace hand {
namespace saga {

/// Create the hand-player saga event router.
/// Reacts to PotAwarded events from Hand domain.
/// Sends DepositFunds commands to Player domain.
angzarr::EventRouter create_hand_player_router();

/// Prepare handler: declare all winners as destinations.
std::vector<angzarr::Cover> prepare_pot_awarded(const examples::PotAwarded& event);

/// Handle PotAwarded: produce DepositFunds commands for each winner.
angzarr::CommandBook handle_pot_awarded(
    const examples::PotAwarded& event,
    const std::vector<angzarr::EventBook>& destinations);

} // namespace saga
} // namespace hand
