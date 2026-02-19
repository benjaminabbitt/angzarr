#pragma once

#include <functional>
#include <map>
#include <string>
#include <vector>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/saga.pb.h"
#include "helpers.hpp"
#include "errors.hpp"
#include "router.hpp"
#include "descriptor.hpp"

namespace angzarr {

/**
 * Base class for sagas using macro-based handler registration.
 *
 * Usage:
 *   class OrderFulfillmentSaga : public Saga {
 *   public:
 *       ANGZARR_SAGA("saga-order-fulfillment", "order", "fulfillment")
 *
 *       ANGZARR_PREPARES(OrderCompleted)
 *       std::vector<Cover> prepare_OrderCompleted(const OrderCompleted& event) {
 *           Cover cover;
 *           cover.set_domain("fulfillment");
 *           cover.mutable_root()->set_value(event.fulfillment_id());
 *           return {cover};
 *       }
 *
 *       ANGZARR_REACTS_TO(OrderCompleted)
 *       CreateShipment handle_OrderCompleted(const OrderCompleted& event) {
 *           CreateShipment cmd;
 *           cmd.set_order_id(event.order_id());
 *           return cmd;
 *       }
 *   };
 */
class Saga {
public:
    using EventDispatcher = std::function<std::vector<CommandBook>(
        Saga*, const google::protobuf::Any&, const std::string&)>;
    using PrepareDispatcher = std::function<std::vector<Cover>(
        Saga*, const google::protobuf::Any&)>;

    virtual ~Saga() = default;

    /**
     * Get the saga name.
     */
    virtual std::string name() const = 0;

    /**
     * Get the input domain.
     */
    virtual std::string input_domain() const = 0;

    /**
     * Get the output domain.
     */
    virtual std::string output_domain() const = 0;

    /**
     * Get destinations needed for source events (two-phase protocol).
     */
    std::vector<Cover> prepare_destinations(const EventBook& book) {
        std::vector<Cover> destinations;

        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;

            auto suffix = helpers::type_name_from_url(page.event().type_url());
            auto it = prepare_handlers().find(suffix);
            if (it != prepare_handlers().end()) {
                auto covers = it->second(this, page.event());
                destinations.insert(destinations.end(), covers.begin(), covers.end());
            }
        }
        return destinations;
    }

    /**
     * Dispatch all events to handlers.
     */
    std::vector<CommandBook> dispatch(const EventBook& book,
                                       const std::vector<EventBook>& destinations = {}) {
        auto correlation_id = book.has_cover() ? book.cover().correlation_id() : "";

        std::vector<CommandBook> commands;
        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;

            auto suffix = helpers::type_name_from_url(page.event().type_url());
            auto it = handlers().find(suffix);
            if (it != handlers().end()) {
                auto cmds = it->second(this, page.event(), correlation_id);
                commands.insert(commands.end(), cmds.begin(), cmds.end());
            }
        }
        return commands;
    }

    /**
     * Build a component descriptor.
     */
    Descriptor descriptor() const {
        std::vector<std::string> types;
        for (const auto& [suffix, _] : handlers()) {
            types.push_back(suffix);
        }
        return {name(), component_types::SAGA, {{input_domain(), types}}};
    }

protected:
    /**
     * Pack a single command into a CommandBook.
     */
    template<typename T>
    std::vector<CommandBook> pack_commands(const T& command, const std::string& correlation_id) {
        CommandBook book;
        auto* cover = book.mutable_cover();
        cover->set_domain(output_domain());
        cover->set_correlation_id(correlation_id);

        auto* page = book.add_pages();
        page->mutable_command()->PackFrom(command, "type.googleapis.com/");

        return {book};
    }

    /**
     * Pack multiple commands into CommandBooks.
     */
    template<typename T>
    std::vector<CommandBook> pack_commands(const std::vector<T>& commands,
                                           const std::string& correlation_id) {
        std::vector<CommandBook> books;
        for (const auto& cmd : commands) {
            auto packed = pack_commands(cmd, correlation_id);
            books.insert(books.end(), packed.begin(), packed.end());
        }
        return books;
    }

    /**
     * Register an event handler (called by ANGZARR_REACTS_TO macro).
     */
    static void register_event_handler(const std::string& suffix, EventDispatcher dispatcher) {
        handlers()[suffix] = std::move(dispatcher);
    }

    /**
     * Register a prepare handler (called by ANGZARR_PREPARES macro).
     */
    static void register_prepare_handler(const std::string& suffix, PrepareDispatcher dispatcher) {
        prepare_handlers()[suffix] = std::move(dispatcher);
    }

private:
    static std::map<std::string, EventDispatcher>& handlers() {
        static std::map<std::string, EventDispatcher> h;
        return h;
    }

    static std::map<std::string, PrepareDispatcher>& prepare_handlers() {
        static std::map<std::string, PrepareDispatcher> p;
        return p;
    }
};

} // namespace angzarr
