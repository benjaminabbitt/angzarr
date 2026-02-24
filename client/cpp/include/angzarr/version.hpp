#pragma once

/**
 * Angzarr C++ Client Library Version
 *
 * Version is injected at build time from VERSION file via CMake.
 */

#ifndef ANGZARR_VERSION
#define ANGZARR_VERSION "dev"
#endif

namespace angzarr {

/// Client library version string
inline constexpr const char* version() { return ANGZARR_VERSION; }

}  // namespace angzarr
