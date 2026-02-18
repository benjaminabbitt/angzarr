#pragma once

#include <string>
#include <vector>
#include "errors.hpp"

namespace angzarr {
namespace validation {

/**
 * Require that an aggregate exists (has prior events).
 */
inline void require_exists(bool exists, const std::string& message = "Aggregate does not exist") {
    if (!exists) {
        throw CommandRejectedError(message);
    }
}

/**
 * Require that an aggregate does not exist.
 */
inline void require_not_exists(bool exists, const std::string& message = "Aggregate already exists") {
    if (exists) {
        throw CommandRejectedError(message);
    }
}

/**
 * Require that a value is positive (greater than zero).
 */
template<typename T>
void require_positive(T value, const std::string& field_name = "value") {
    if (value <= 0) {
        throw CommandRejectedError(field_name + " must be positive");
    }
}

/**
 * Require that a value is non-negative (zero or greater).
 */
template<typename T>
void require_non_negative(T value, const std::string& field_name = "value") {
    if (value < 0) {
        throw CommandRejectedError(field_name + " must be non-negative");
    }
}

/**
 * Require that a string is not empty.
 */
inline void require_not_empty(const std::string& value, const std::string& field_name = "value") {
    if (value.empty()) {
        throw CommandRejectedError(field_name + " must not be empty");
    }
}

/**
 * Require that a collection is not empty.
 */
template<typename T>
void require_not_empty(const std::vector<T>& collection, const std::string& field_name = "collection") {
    if (collection.empty()) {
        throw CommandRejectedError(field_name + " must not be empty");
    }
}

/**
 * Require that a status matches an expected value.
 */
template<typename T>
void require_status(T actual, T expected, const std::string& message = "Invalid status") {
    if (actual != expected) {
        throw CommandRejectedError(message);
    }
}

/**
 * Require that a status does not match a forbidden value.
 */
template<typename T>
void require_status_not(T actual, T forbidden, const std::string& message = "Invalid status") {
    if (actual == forbidden) {
        throw CommandRejectedError(message);
    }
}

} // namespace validation
} // namespace angzarr
