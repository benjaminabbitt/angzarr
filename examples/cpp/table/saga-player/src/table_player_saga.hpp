#pragma once

#include "angzarr/event_router.hpp"
#include "angzarr/types.pb.h"
#include "examples/table.pb.h"
#include "examples/player.pb.h"

namespace table {
namespace saga {

/// Create the table-player saga event router.
/// Reacts to HandEnded events from Table domain.
/// Sends ReleaseFunds commands to Player domain.
angzarr::EventRouter create_table_player_router();

/// Prepare handler: declare all players in stack_changes as destinations.
std::vector<angzarr::Cover> prepare_hand_ended(const examples::HandEnded& event);

/// Handle HandEnded: produce ReleaseFunds commands for each player.
angzarr::CommandBook handle_hand_ended(
    const examples::HandEnded& event,
    const std::vector<angzarr::EventBook>& destinations);

} // namespace saga
} // namespace table
