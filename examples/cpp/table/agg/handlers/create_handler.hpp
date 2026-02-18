#pragma once

#include "table_state.hpp"
#include "examples/table.pb.h"

namespace table {
namespace handlers {

/// Handle CreateTable command.
examples::TableCreated handle_create(
    const examples::CreateTable& cmd,
    const TableState& state);

} // namespace handlers
} // namespace table
