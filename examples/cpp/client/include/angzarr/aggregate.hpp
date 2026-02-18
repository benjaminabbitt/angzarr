#pragma once

#include <functional>
#include <memory>
#include <string>
#include <unordered_map>
#include <google/protobuf/message.h>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "errors.hpp"

namespace angzarr {

/// Base class for event-sourced aggregates using CRTP pattern.
/// Derived classes must implement:
///   - void apply_event(State& state, const google::protobuf::Any& event)
///   - Register handlers via register_handler<CommandType>(handler)
template<typename Derived, typename State>
class Aggregate {
public:
    using MessagePtr = std::unique_ptr<google::protobuf::Message>;
    using CommandHandler = std::function<MessagePtr(const google::protobuf::Message&, const State&)>;

protected:
    State state_;
    std::unordered_map<std::string, CommandHandler> handlers_;

public:
    Aggregate() : state_{} {}
    virtual ~Aggregate() = default;

    /// Rehydrate aggregate from event history.
    void rehydrate(const EventBook& event_book) {
        state_ = State{};
        for (const auto& page : event_book.pages()) {
            static_cast<Derived*>(this)->apply_event(state_, page.event());
        }
    }

    /// Handle a command using registered handlers.
    template<typename CommandType>
    MessagePtr handle_command(const CommandType& cmd) {
        const std::string type_name = CommandType::descriptor()->full_name();
        auto it = handlers_.find(type_name);
        if (it == handlers_.end()) {
            throw std::runtime_error("No handler for command type: " + type_name);
        }
        return it->second(cmd, state_);
    }

    /// Get the current state (const reference).
    const State& state() const { return state_; }

protected:
    /// Register a command handler.
    template<typename CommandType, typename EventType>
    void register_handler(std::function<EventType(const CommandType&, const State&)> handler) {
        const std::string type_name = CommandType::descriptor()->full_name();
        handlers_[type_name] = [handler](const google::protobuf::Message& msg, const State& state) -> MessagePtr {
            const auto& cmd = static_cast<const CommandType&>(msg);
            auto event = handler(cmd, state);
            return std::make_unique<EventType>(std::move(event));
        };
    }

    /// Helper to create current timestamp.
    static google::protobuf::Timestamp now() {
        google::protobuf::Timestamp ts;
        auto now = std::chrono::system_clock::now();
        auto seconds = std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch()).count();
        ts.set_seconds(seconds);
        return ts;
    }
};

} // namespace angzarr
