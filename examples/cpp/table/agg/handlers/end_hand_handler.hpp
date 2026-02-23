#pragma once

#include "examples/table.pb.h"
#include "table_state.hpp"

namespace table {
namespace handlers {

/// Handle EndHand command.
examples::HandEnded handle_end_hand(const examples::EndHand& cmd, const TableState& state);

}  // namespace handlers
}  // namespace table
