#pragma once

#include <functional>
#include <string>
#include <vector>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"

namespace angzarr {

/**
 * Handler type for upcasting events from old versions to new versions.
 *
 * Takes an old event (Any) and returns the new event (Any).
 */
using UpcasterHandler = std::function<google::protobuf::Any(const google::protobuf::Any&)>;

/**
 * Event version transformer.
 *
 * Matches old event type_url suffixes and transforms to new versions.
 * Events without registered transformations pass through unchanged.
 *
 * Example:
 * @code
 * auto router = UpcasterRouter("order")
 *     .on("OrderCreatedV1", [](const Any& old) {
 *         OrderCreatedV1 v1;
 *         old.UnpackTo(&v1);
 *         OrderCreated v2;
 *         v2.set_order_id(v1.order_id());
 *         Any result;
 *         result.PackFrom(v2);
 *         return result;
 *     });
 *
 * auto new_events = router.upcast(old_events);
 * @endcode
 */
class UpcasterRouter {
public:
    /**
     * Create a new upcaster router for a domain.
     *
     * @param domain The domain this upcaster handles
     */
    explicit UpcasterRouter(const std::string& domain)
        : domain_(domain) {}

    /**
     * Register a handler for an old event type_url suffix.
     *
     * The suffix is matched against the end of the event's type_url.
     * For example, suffix "OrderCreatedV1" matches
     * "type.googleapis.com/examples.OrderCreatedV1".
     *
     * @param suffix The type_url suffix to match
     * @param handler Function that transforms old event to new event
     * @return Reference to this router for fluent chaining
     */
    UpcasterRouter& on(const std::string& suffix, UpcasterHandler handler) {
        handlers_.emplace_back(suffix, std::move(handler));
        return *this;
    }

    /**
     * Transform a list of events to current versions.
     *
     * Events matching registered handlers are transformed.
     * Events without matching handlers pass through unchanged.
     *
     * @param events Vector of EventPages to transform
     * @return Vector of EventPages with transformed events
     */
    std::vector<EventPage> upcast(const std::vector<EventPage>& events) const {
        std::vector<EventPage> result;
        result.reserve(events.size());

        for (const auto& page : events) {
            if (!page.has_event()) {
                result.push_back(page);
                continue;
            }

            const auto& event = page.event();
            const std::string& type_url = event.type_url();
            bool transformed = false;

            for (const auto& [suffix, handler] : handlers_) {
                if (type_url.size() >= suffix.size() &&
                    type_url.compare(type_url.size() - suffix.size(), suffix.size(), suffix) == 0) {
                    auto new_event = handler(event);
                    EventPage new_page;
                    new_page.mutable_event()->CopyFrom(new_event);
                    new_page.set_sequence(page.sequence());
                    if (page.has_created_at()) {
                        new_page.mutable_created_at()->CopyFrom(page.created_at());
                    }
                    result.push_back(std::move(new_page));
                    transformed = true;
                    break;
                }
            }

            if (!transformed) {
                result.push_back(page);
            }
        }

        return result;
    }

    /**
     * Get the domain this upcaster handles.
     *
     * @return The domain name
     */
    const std::string& domain() const {
        return domain_;
    }

private:
    std::string domain_;
    std::vector<std::pair<std::string, UpcasterHandler>> handlers_;
};

} // namespace angzarr
