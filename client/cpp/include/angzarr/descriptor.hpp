#pragma once

#include <map>
#include <string>
#include <vector>

namespace angzarr {

/**
 * Component type constants for topology discovery.
 */
namespace component_types {
    constexpr const char* AGGREGATE = "aggregate";
    constexpr const char* SAGA = "saga";
    constexpr const char* PROJECTOR = "projector";
    constexpr const char* PROCESS_MANAGER = "process_manager";
}

/**
 * Component descriptor for topology registration.
 * Describes a component's name, type, and input subscriptions.
 */
struct Descriptor {
    std::string name;
    std::string component_type;
    std::map<std::string, std::vector<std::string>> inputs;
};

} // namespace angzarr
