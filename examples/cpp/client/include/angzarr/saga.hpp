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

/// Base class for sagas using CRTP pattern.
/// Sagas translate events from one domain into commands for another domain.
template<typename Derived>
class Saga {
public:
    using PrepareHandler = std::function<std::vector<Cover>(const google::protobuf::Message&)>;
    using ReactHandler = std::function<CommandBook(const google::protobuf::Message&, const std::vector<EventBook>&)>;

protected:
    std::unordered_map<std::string, PrepareHandler> prepare_handlers_;
    std::unordered_map<std::string, ReactHandler> react_handlers_;

public:
    virtual ~Saga() = default;

    /// Get the saga name.
    virtual std::string name() const = 0;

    /// Get the input domain this saga listens to.
    virtual std::string input_domain() const = 0;

    /// Prepare phase - declare destinations needed.
    template<typename EventType>
    std::vector<Cover> prepare(const EventType& event) {
        const std::string type_name = EventType::descriptor()->full_name();
        auto it = prepare_handlers_.find(type_name);
        if (it == prepare_handlers_.end()) {
            return {};
        }
        return it->second(event);
    }

    /// Handle phase - process event and produce commands.
    template<typename EventType>
    CommandBook handle(const EventType& event, const std::vector<EventBook>& destinations) {
        const std::string type_name = EventType::descriptor()->full_name();
        auto it = react_handlers_.find(type_name);
        if (it == react_handlers_.end()) {
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

protected:
    /// Register a prepare handler.
    template<typename EventType>
    void register_prepare(std::function<std::vector<Cover>(const EventType&)> handler) {
        const std::string type_name = EventType::descriptor()->full_name();
        prepare_handlers_[type_name] = [handler](const google::protobuf::Message& msg) {
            const auto& event = static_cast<const EventType&>(msg);
            return handler(event);
        };
    }

    /// Register a react handler.
    template<typename EventType>
    void register_react(std::function<CommandBook(const EventType&, const std::vector<EventBook>&)> handler) {
        const std::string type_name = EventType::descriptor()->full_name();
        react_handlers_[type_name] = [handler](const google::protobuf::Message& msg,
                                                const std::vector<EventBook>& destinations) {
            const auto& event = static_cast<const EventType&>(msg);
            return handler(event, destinations);
        };
    }
};

} // namespace angzarr
