#pragma once

#include <functional>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>
#include <google/protobuf/message.h>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"

namespace angzarr {

/// Functional router for saga event handling.
/// Two-phase pattern: prepare (get destinations) then handle (produce commands).
/// Uses fluent .domain().on() pattern to register handlers with domain context.
class EventRouter {
public:
    using PrepareHandler = std::function<std::vector<Cover>(const google::protobuf::Message&)>;
    using HandleHandler = std::function<CommandBook(const google::protobuf::Message&, const std::vector<EventBook>&)>;

private:
    std::string name_;
    std::string current_domain_;
    std::unordered_map<std::string, PrepareHandler> prepare_handlers_;
    std::unordered_map<std::string, HandleHandler> handle_handlers_;

public:
    explicit EventRouter(const std::string& name)
        : name_(name) {}

    /// Set the current domain context for subsequent .on() calls.
    EventRouter& domain(const std::string& domain_name) {
        current_domain_ = domain_name;
        return *this;
    }

    /// Register a prepare handler.
    template<typename EventType>
    EventRouter& prepare(std::function<std::vector<Cover>(const EventType&)> handler) {
        const std::string type_name = EventType::descriptor()->full_name();
        prepare_handlers_[type_name] = [handler](const google::protobuf::Message& msg) {
            const auto& event = static_cast<const EventType&>(msg);
            return handler(event);
        };
        return *this;
    }

    /// Register an event reaction handler.
    template<typename EventType>
    EventRouter& on(std::function<CommandBook(const EventType&, const std::vector<EventBook>&)> handler) {
        const std::string type_name = EventType::descriptor()->full_name();
        handle_handlers_[type_name] = [handler](const google::protobuf::Message& msg,
                                                 const std::vector<EventBook>& destinations) {
            const auto& event = static_cast<const EventType&>(msg);
            return handler(event, destinations);
        };
        return *this;
    }

    /// Execute prepare phase - get destination covers.
    template<typename EventType>
    std::vector<Cover> do_prepare(const EventType& event) {
        const std::string type_name = EventType::descriptor()->full_name();
        auto it = prepare_handlers_.find(type_name);
        if (it == prepare_handlers_.end()) {
            return {};
        }
        return it->second(event);
    }

    /// Execute handle phase - produce commands.
    template<typename EventType>
    CommandBook do_handle(const EventType& event, const std::vector<EventBook>& destinations) {
        const std::string type_name = EventType::descriptor()->full_name();
        auto it = handle_handlers_.find(type_name);
        if (it == handle_handlers_.end()) {
            return CommandBook{};
        }
        return it->second(event, destinations);
    }

    /// Get next sequence number from an event book.
    static int next_sequence(const EventBook* event_book) {
        if (event_book == nullptr) return 0;
        return event_book->next_sequence();
    }

    /// Pack a command message into Any.
    template<typename CommandType>
    static google::protobuf::Any pack_command(const CommandType& cmd) {
        google::protobuf::Any any;
        any.PackFrom(cmd, "type.googleapis.com/");
        return any;
    }

    const std::string& name() const { return name_; }
};

} // namespace angzarr
