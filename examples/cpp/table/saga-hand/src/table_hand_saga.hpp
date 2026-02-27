#pragma once

#include "angzarr/router.hpp"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/table.pb.h"

namespace table {
namespace saga {

/// Create the table-hand saga event router.
/// Reacts to HandStarted events from Table domain.
/// Sends DealCards commands to Hand domain.
angzarr::EventRouter create_table_hand_router();

// Note: Handlers are internal to the router - only create_table_hand_router is public

}  // namespace saga
}  // namespace table
