#pragma once

#include <google/protobuf/any.pb.h>

#include <functional>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "angzarr/cloudevents.pb.h"
#include "angzarr/types.pb.h"
#include "descriptor.hpp"
#include "helpers.hpp"

namespace angzarr {

// ============================================================================
// CloudEvents Projector (OO Pattern)
// ============================================================================

/**
 * Base class for OO-style CloudEvents projectors.
 *
 * Implement this class and add `on_{event_type}` methods to create
 * a CloudEvents projector using the OO pattern.
 *
 * Example:
 * @code
 *   class PlayerCloudEventsProjector : public CloudEventsProjector {
 *   public:
 *       PlayerCloudEventsProjector()
 *           : CloudEventsProjector("prj-player-cloudevents", "player") {}
 *
 *       std::optional<CloudEvent> on_player_registered(const PlayerRegistered& event) {
 *           CloudEvent ce;
 *           ce.set_type("com.poker.player.registered");
 *           ce.mutable_data()->PackFrom(public_event);
 *           return ce;
 *       }
 *   };
 * @endcode
 */
class CloudEventsProjector {
   public:
    /**
     * Create a CloudEvents projector.
     *
     * @param name The projector name (e.g., "prj-player-cloudevents")
     * @param domain The input domain (e.g., "player")
     */
    CloudEventsProjector(std::string name, std::string domain)
        : name_(std::move(name)), domain_(std::move(domain)) {}

    virtual ~CloudEventsProjector() = default;

    /**
     * Get the projector name.
     */
    const std::string& name() const { return name_; }

    /**
     * Get the input domain.
     */
    const std::string& domain() const { return domain_; }

    /**
     * Build a component descriptor for topology registration.
     */
    Descriptor descriptor() const { return {name_, component_types::PROJECTOR, {{domain_, {}}}}; }

   private:
    std::string name_;
    std::string domain_;
};

// ============================================================================
// CloudEvents Router (Functional Pattern)
// ============================================================================

/// Handler function type for CloudEvents transformation.
template <typename T>
using CloudEventsHandler = std::function<std::optional<CloudEvent>(const T&)>;

/**
 * Functional router for CloudEvents projectors.
 *
 * Provides a fluent builder API for registering event handlers that
 * transform domain events into CloudEvents.
 *
 * Example:
 * @code
 *   auto router = CloudEventsRouter("prj-player-cloudevents", "player")
 *       .on<PlayerRegistered>(handle_player_registered)
 *       .on<FundsDeposited>(handle_funds_deposited);
 *
 *   run_cloudevents_projector("prj-player-cloudevents", 50091, router);
 * @endcode
 */
class CloudEventsRouter {
   public:
    using BoxedHandler = std::function<std::optional<CloudEvent>(const google::protobuf::Any&)>;

    /**
     * Create a new CloudEvents router.
     *
     * @param name The projector name (e.g., "prj-player-cloudevents")
     * @param domain The input domain (e.g., "player")
     */
    CloudEventsRouter(std::string name, std::string domain)
        : name_(std::move(name)), domain_(std::move(domain)) {}

    /**
     * Register a handler for an event type.
     *
     * The event type is automatically inferred from the handler's parameter type.
     *
     * @tparam T The protobuf event type
     * @param handler Function that transforms the event into a CloudEvent
     * @return Reference to this router for chaining
     *
     * Example:
     * @code
     *   router.on<PlayerRegistered>([](const PlayerRegistered& event) {
     *       CloudEvent ce;
     *       ce.set_type("com.poker.player.registered");
     *       return ce;
     *   });
     * @endcode
     */
    template <typename T>
    CloudEventsRouter& on(CloudEventsHandler<T> handler) {
        std::string suffix = T::descriptor()->name();
        handlers_[suffix] = [handler = std::move(handler)](
                                const google::protobuf::Any& any) -> std::optional<CloudEvent> {
            T event;
            if (!any.UnpackTo(&event)) {
                return std::nullopt;
            }
            return handler(event);
        };
        return *this;
    }

    /**
     * Get the projector name.
     */
    const std::string& name() const { return name_; }

    /**
     * Get the input domain.
     */
    const std::string& domain() const { return domain_; }

    /**
     * Get the event types this router handles.
     */
    std::vector<std::string> event_types() const {
        std::vector<std::string> types;
        for (const auto& [suffix, _] : handlers_) {
            types.push_back(suffix);
        }
        return types;
    }

    /**
     * Build a component descriptor for topology registration.
     */
    Descriptor descriptor() const {
        return {name_, component_types::PROJECTOR, {{domain_, event_types()}}};
    }

    /**
     * Project an EventBook into CloudEvents.
     *
     * @param source The source EventBook containing domain events
     * @return CloudEventsResponse containing the transformed CloudEvents
     */
    CloudEventsResponse project(const EventBook& source) const {
        CloudEventsResponse response;

        for (const auto& page : source.pages()) {
            if (!page.has_event()) continue;

            const auto& event_any = page.event();
            std::string suffix = helpers::type_name_from_url(event_any.type_url());

            auto it = handlers_.find(suffix);
            if (it != handlers_.end()) {
                auto cloud_event = it->second(event_any);
                if (cloud_event.has_value()) {
                    *response.add_events() = std::move(*cloud_event);
                }
            }
        }

        return response;
    }

   private:
    std::string name_;
    std::string domain_;
    std::map<std::string, BoxedHandler> handlers_;
};

// ============================================================================
// Server Function
// ============================================================================

/**
 * Run a CloudEvents projector service.
 *
 * CloudEvents projectors transform domain events into CloudEvents 1.0 format
 * for external consumption via HTTP webhooks or Kafka.
 *
 * @param name The projector name
 * @param port The port to listen on
 * @param router The CloudEvents router
 *
 * Example:
 * @code
 *   auto router = CloudEventsRouter("prj-player-cloudevents", "player")
 *       .on<PlayerRegistered>(handle_player_registered);
 *
 *   run_cloudevents_projector("prj-player-cloudevents", 50091, router);
 * @endcode
 */
void run_cloudevents_projector(const std::string& name, int port, const CloudEventsRouter& router);

}  // namespace angzarr
