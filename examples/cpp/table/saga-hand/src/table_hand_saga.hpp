#pragma once

#include "angzarr/event_router.hpp"
#include "angzarr/types.pb.h"
#include "examples/table.pb.h"
#include "examples/hand.pb.h"

namespace table {
namespace saga {

/// Create the table-hand saga event router.
/// Reacts to HandStarted events from Table domain.
/// Sends DealCards commands to Hand domain.
angzarr::EventRouter create_table_hand_router();

/// Prepare handler: declare destination for HandStarted event.
std::vector<angzarr::Cover> prepare_hand_started(const examples::HandStarted& event);

/// Handle HandStarted: produce DealCards command.
angzarr::CommandBook handle_hand_started(
    const examples::HandStarted& event,
    const std::vector<angzarr::EventBook>& destinations);

} // namespace saga
} // namespace table
