#pragma once

#include "angzarr/event_router.hpp"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/table.pb.h"

namespace hand {
namespace saga {

/// Create the hand-table saga event router.
/// Reacts to HandComplete events from Hand domain.
/// Sends EndHand commands to Table domain.
angzarr::EventRouter create_hand_table_router();

/// Prepare handler: declare the table aggregate as destination.
std::vector<angzarr::Cover> prepare_hand_complete(const examples::HandComplete& event);

/// Handle HandComplete: produce EndHand command.
angzarr::CommandBook handle_hand_complete(
    const examples::HandComplete& event,
    const std::vector<angzarr::EventBook>& destinations);

} // namespace saga
} // namespace hand
