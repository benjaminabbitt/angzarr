#pragma once

#include "table_state.hpp"
#include "examples/table.pb.h"

namespace table {
namespace handlers {

/// Handle StartHand command.
examples::HandStarted handle_start_hand(
    const examples::StartHand& cmd,
    const TableState& state);

} // namespace handlers
} // namespace table
