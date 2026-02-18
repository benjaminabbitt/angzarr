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

/// Functional router for aggregate command handling.
/// Uses a state builder function to reconstruct state from EventBook on each dispatch.
template<typename State>
class CommandRouter {
public:
    using MessagePtr = std::unique_ptr<google::protobuf::Message>;
    using StateBuilder = std::function<State(const EventBook&)>;
    using CommandHandler = std::function<MessagePtr(const google::protobuf::Message&, const State&)>;
    using RejectionHandler = std::function<MessagePtr(const Notification&, const State&)>;
    using RejectionKey = std::pair<std::string, std::string>; // (domain, command)

    struct RejectionKeyHash {
        size_t operator()(const RejectionKey& k) const {
            return std::hash<std::string>()(k.first) ^ (std::hash<std::string>()(k.second) << 1);
        }
    };

private:
    std::string name_;
    StateBuilder state_builder_;
    std::unordered_map<std::string, CommandHandler> handlers_;
    std::unordered_map<RejectionKey, RejectionHandler, RejectionKeyHash> rejection_handlers_;

public:
    CommandRouter(const std::string& name, StateBuilder state_builder)
        : name_(name), state_builder_(std::move(state_builder)) {}

    /// Register a command handler.
    template<typename CommandType, typename EventType>
    CommandRouter& on(std::function<EventType(const CommandType&, const State&)> handler) {
        const std::string type_name = CommandType::descriptor()->full_name();
        handlers_[type_name] = [handler](const google::protobuf::Message& msg, const State& state) -> MessagePtr {
            const auto& cmd = static_cast<const CommandType&>(msg);
            auto event = handler(cmd, state);
            return std::make_unique<EventType>(std::move(event));
        };
        return *this;
    }

    /// Register a rejection handler for compensation.
    CommandRouter& on_rejected(const std::string& domain, const std::string& command, RejectionHandler handler) {
        rejection_handlers_[{domain, command}] = std::move(handler);
        return *this;
    }

    /// Dispatch a command.
    template<typename CommandType>
    MessagePtr dispatch(const CommandType& cmd, const EventBook& event_book) {
        const std::string type_name = CommandType::descriptor()->full_name();
        auto it = handlers_.find(type_name);
        if (it == handlers_.end()) {
            throw std::runtime_error("No handler for command type: " + type_name);
        }
        State state = state_builder_(event_book);
        return it->second(cmd, state);
    }

    /// Handle a rejection notification.
    MessagePtr handle_rejection(const Notification& notification, const EventBook& event_book) {
        // Extract domain and command from notification payload
        std::string domain;
        std::string command;

        if (notification.has_payload() &&
            notification.payload().type_url().find("RejectionNotification") != std::string::npos) {
            RejectionNotification rejection;
            notification.payload().UnpackTo(&rejection);

            if (rejection.has_rejected_command()) {
                const auto& cmd_book = rejection.rejected_command();
                if (cmd_book.pages_size() > 0) {
                    const auto& cmd_any = cmd_book.pages(0).command();
                    // Parse type URL to get command type
                    const std::string& type_url = cmd_any.type_url();
                    size_t pos = type_url.rfind('/');
                    if (pos != std::string::npos) {
                        command = type_url.substr(pos + 1);
                    }
                }
                domain = cmd_book.cover().domain();
            }
        }

        RejectionKey key{domain, command};
        auto it = rejection_handlers_.find(key);
        if (it == rejection_handlers_.end()) {
            return nullptr; // Delegate to framework
        }
        State state = state_builder_(event_book);
        return it->second(notification, state);
    }

    const std::string& name() const { return name_; }
};

} // namespace angzarr
