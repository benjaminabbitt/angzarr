#pragma once

#include <google/protobuf/any.pb.h>
#include <google/protobuf/message.h>

#include <functional>
#include <memory>
#include <string>
#include <unordered_map>
#include <variant>
#include <vector>

#include "angzarr/types.pb.h"

namespace angzarr {

/// Response from saga handlers - can contain commands and/or facts.
struct SagaHandlerResponse {
    std::vector<CommandBook> commands;
    std::vector<EventBook> events;  // Facts to inject into target aggregates

    static SagaHandlerResponse empty() { return {{}, {}}; }

    static SagaHandlerResponse with_commands(std::vector<CommandBook> cmds) {
        return {std::move(cmds), {}};
    }

    static SagaHandlerResponse with_events(std::vector<EventBook> evts) {
        return {{}, std::move(evts)};
    }

    static SagaHandlerResponse with_both(std::vector<CommandBook> cmds,
                                         std::vector<EventBook> evts) {
        return {std::move(cmds), std::move(evts)};
    }
};

/// Functional router for saga event handling.
/// Two-phase pattern: prepare (get destinations) then handle (produce commands/facts).
/// Uses fluent .domain().on() pattern to register handlers with domain context.
///
/// Why two-phase? Sagas need destination state to compute correct sequences.
/// Prepare declares which EventBooks are needed; the framework fetches them;
/// Handle receives destinations and uses next_sequence() for commands.
class EventRouter {
   public:
    using PrepareHandler = std::function<std::vector<Cover>(const google::protobuf::Message&)>;
    using HandleHandler = std::function<SagaHandlerResponse(const google::protobuf::Message&,
                                                            const std::vector<EventBook>&)>;

   private:
    std::string name_;
    std::string current_domain_;
    std::unordered_map<std::string, PrepareHandler> prepare_handlers_;
    std::unordered_map<std::string, HandleHandler> handle_handlers_;

   public:
    explicit EventRouter(const std::string& name) : name_(name) {}

    /// Set the current domain context for subsequent .on() calls.
    EventRouter& domain(const std::string& domain_name) {
        current_domain_ = domain_name;
        return *this;
    }

    /// Register a prepare handler.
    template <typename EventType>
    EventRouter& prepare(std::function<std::vector<Cover>(const EventType&)> handler) {
        const std::string type_name = EventType::descriptor()->full_name();
        prepare_handlers_[type_name] = [handler](const google::protobuf::Message& msg) {
            const auto& event = static_cast<const EventType&>(msg);
            return handler(event);
        };
        return *this;
    }

    /// Register an event reaction handler.
    template <typename EventType>
    EventRouter& on(
        std::function<SagaHandlerResponse(const EventType&, const std::vector<EventBook>&)>
            handler) {
        const std::string type_name = EventType::descriptor()->full_name();
        handle_handlers_[type_name] = [handler](const google::protobuf::Message& msg,
                                                const std::vector<EventBook>& destinations) {
            const auto& event = static_cast<const EventType&>(msg);
            return handler(event, destinations);
        };
        return *this;
    }

    /// Execute prepare phase - get destination covers.
    template <typename EventType>
    std::vector<Cover> do_prepare(const EventType& event) {
        const std::string type_name = EventType::descriptor()->full_name();
        auto it = prepare_handlers_.find(type_name);
        if (it == prepare_handlers_.end()) {
            return {};
        }
        return it->second(event);
    }

    /// Execute handle phase - produce commands and/or facts.
    template <typename EventType>
    SagaHandlerResponse do_handle(const EventType& event,
                                  const std::vector<EventBook>& destinations) {
        const std::string type_name = EventType::descriptor()->full_name();
        auto it = handle_handlers_.find(type_name);
        if (it == handle_handlers_.end()) {
            return SagaHandlerResponse::empty();
        }
        return it->second(event, destinations);
    }

    /// Get next sequence number from an event book.
    static int next_sequence(const EventBook* event_book) {
        if (event_book == nullptr) return 0;
        return event_book->next_sequence();
    }

    /// Pack a command message into Any.
    template <typename CommandType>
    static google::protobuf::Any pack_command(const CommandType& cmd) {
        google::protobuf::Any any;
        any.PackFrom(cmd, "type.googleapis.com/");
        return any;
    }

    const std::string& name() const { return name_; }
};

}  // namespace angzarr
