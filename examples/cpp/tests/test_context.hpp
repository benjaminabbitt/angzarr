#pragma once

#include <google/protobuf/any.pb.h>
#include <grpcpp/grpcpp.h>

#include <optional>
#include <string>
#include <vector>

#include "angzarr/types.pb.h"
#include "hand_state.hpp"
#include "player_state.hpp"
#include "table_state.hpp"

namespace tests {

/// Shared scenario context for BDD acceptance tests.
///
/// Each scenario gets a fresh context. The context holds:
/// - Event pages accumulated during the scenario
/// - Domain-specific state rebuilt from events
/// - Result of the last command handler invocation
/// - Any error from the last command
struct ScenarioContext {
    // Event history for state reconstruction
    std::vector<angzarr::EventPage> event_pages;

    // Domain-specific state (rebuilt from events)
    player::PlayerState player_state;
    table::TableState table_state;
    hand::HandState hand_state;

    // Result of last handler invocation
    std::optional<google::protobuf::Any> result_event;

    // Error from last command (if rejected)
    std::optional<std::string> last_error;
    std::optional<grpc::StatusCode> last_error_code;

    // Current sequence number for event pages
    int64_t next_sequence = 1;

    /// Reset context for a new scenario.
    void reset() {
        event_pages.clear();
        player_state = player::PlayerState{};
        table_state = table::TableState{};
        hand_state = hand::HandState{};
        result_event.reset();
        last_error.reset();
        last_error_code.reset();
        next_sequence = 1;
    }

    /// Add an event to the event history.
    template <typename EventT>
    void add_event(const EventT& event) {
        angzarr::EventPage page;
        page.set_sequence(next_sequence++);
        page.mutable_event()->PackFrom(event);
        event_pages.push_back(std::move(page));
    }

    /// Set the result event from a handler.
    template <typename EventT>
    void set_result(const EventT& event) {
        result_event.emplace();
        result_event->PackFrom(event);
    }

    /// Set error from a rejected command.
    void set_error(const std::string& message, grpc::StatusCode code) {
        last_error = message;
        last_error_code = code;
        result_event.reset();
    }

    /// Clear any previous error.
    void clear_error() {
        last_error.reset();
        last_error_code.reset();
    }

    /// Check if the last command failed.
    bool has_error() const { return last_error.has_value(); }

    /// Get the result event unpacked to a specific type.
    template <typename EventT>
    std::optional<EventT> get_result_as() const {
        if (!result_event.has_value()) {
            return std::nullopt;
        }
        EventT event;
        if (result_event->UnpackTo(&event)) {
            return event;
        }
        return std::nullopt;
    }

    /// Rebuild player state from event history.
    void rebuild_player_state() {
        angzarr::EventBook book;
        for (const auto& page : event_pages) {
            *book.add_pages() = page;
        }
        player_state = player::PlayerState::from_event_book(book);
    }

    /// Rebuild table state from event history.
    void rebuild_table_state() {
        angzarr::EventBook book;
        for (const auto& page : event_pages) {
            *book.add_pages() = page;
        }
        table_state = table::TableState::from_event_book(book);
    }

    /// Rebuild hand state from event history.
    void rebuild_hand_state() {
        angzarr::EventBook book;
        for (const auto& page : event_pages) {
            *book.add_pages() = page;
        }
        hand_state = hand::HandState::from_event_book(book);
    }
};

/// Global scenario context (thread-local for test isolation).
inline thread_local ScenarioContext g_context;

/// Helper to convert gRPC status code to string for assertions.
inline std::string status_code_to_string(grpc::StatusCode code) {
    switch (code) {
        case grpc::StatusCode::OK:
            return "OK";
        case grpc::StatusCode::CANCELLED:
            return "CANCELLED";
        case grpc::StatusCode::UNKNOWN:
            return "UNKNOWN";
        case grpc::StatusCode::INVALID_ARGUMENT:
            return "INVALID_ARGUMENT";
        case grpc::StatusCode::DEADLINE_EXCEEDED:
            return "DEADLINE_EXCEEDED";
        case grpc::StatusCode::NOT_FOUND:
            return "NOT_FOUND";
        case grpc::StatusCode::ALREADY_EXISTS:
            return "ALREADY_EXISTS";
        case grpc::StatusCode::PERMISSION_DENIED:
            return "PERMISSION_DENIED";
        case grpc::StatusCode::RESOURCE_EXHAUSTED:
            return "RESOURCE_EXHAUSTED";
        case grpc::StatusCode::FAILED_PRECONDITION:
            return "FAILED_PRECONDITION";
        case grpc::StatusCode::ABORTED:
            return "ABORTED";
        case grpc::StatusCode::OUT_OF_RANGE:
            return "OUT_OF_RANGE";
        case grpc::StatusCode::UNIMPLEMENTED:
            return "UNIMPLEMENTED";
        case grpc::StatusCode::INTERNAL:
            return "INTERNAL";
        case grpc::StatusCode::UNAVAILABLE:
            return "UNAVAILABLE";
        case grpc::StatusCode::DATA_LOSS:
            return "DATA_LOSS";
        case grpc::StatusCode::UNAUTHENTICATED:
            return "UNAUTHENTICATED";
        default:
            return "UNKNOWN_CODE";
    }
}

/// Helper to parse status code from string.
inline grpc::StatusCode string_to_status_code(const std::string& str) {
    if (str == "OK") return grpc::StatusCode::OK;
    if (str == "CANCELLED") return grpc::StatusCode::CANCELLED;
    if (str == "INVALID_ARGUMENT") return grpc::StatusCode::INVALID_ARGUMENT;
    if (str == "DEADLINE_EXCEEDED") return grpc::StatusCode::DEADLINE_EXCEEDED;
    if (str == "NOT_FOUND") return grpc::StatusCode::NOT_FOUND;
    if (str == "ALREADY_EXISTS") return grpc::StatusCode::ALREADY_EXISTS;
    if (str == "PERMISSION_DENIED") return grpc::StatusCode::PERMISSION_DENIED;
    if (str == "RESOURCE_EXHAUSTED") return grpc::StatusCode::RESOURCE_EXHAUSTED;
    if (str == "FAILED_PRECONDITION") return grpc::StatusCode::FAILED_PRECONDITION;
    if (str == "ABORTED") return grpc::StatusCode::ABORTED;
    if (str == "OUT_OF_RANGE") return grpc::StatusCode::OUT_OF_RANGE;
    if (str == "UNIMPLEMENTED") return grpc::StatusCode::UNIMPLEMENTED;
    if (str == "INTERNAL") return grpc::StatusCode::INTERNAL;
    if (str == "UNAVAILABLE") return grpc::StatusCode::UNAVAILABLE;
    if (str == "DATA_LOSS") return grpc::StatusCode::DATA_LOSS;
    if (str == "UNAUTHENTICATED") return grpc::StatusCode::UNAUTHENTICATED;
    return grpc::StatusCode::UNKNOWN;
}

}  // namespace tests
