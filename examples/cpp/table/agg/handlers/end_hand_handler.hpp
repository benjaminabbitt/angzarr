#pragma once

#include "table_state.hpp"
#include "examples/table.pb.h"

namespace table {
namespace handlers {

/// Handle EndHand command.
examples::HandEnded handle_end_hand(
    const examples::EndHand& cmd,
    const TableState& state);

} // namespace handlers
} // namespace table
