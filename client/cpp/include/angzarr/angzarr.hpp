#pragma once

/**
 * Angzarr C++ Client Library
 *
 * Main include file - includes all public headers.
 */

// Version
#include "version.hpp"

// Error types
#include "errors.hpp"

// Helper utilities
#include "helpers.hpp"

// Validation helpers
#include "validation.hpp"

// Registration macros
#include "macros.hpp"

// Functional router pattern
#include "router.hpp"

// Unified router pattern (trait-based)
#include "handler_traits.hpp"
#include "unified_router.hpp"

// OO base classes with macro registration
#include "command_handler.hpp"
#include "process_manager.hpp"
#include "projector.hpp"
#include "saga.hpp"

// CloudEvents support
#include "cloudevents.hpp"

// gRPC client classes
#include "client.hpp"

// Fluent builders
#include "builder.hpp"

// Compensation context for rejection handling
#include "compensation.hpp"
