#pragma once

#include <functional>
#include <map>
#include <string>
#include <vector>
#include <google/protobuf/any.pb.h>
#include "types.pb.h"
#include "helpers.hpp"
#include "errors.hpp"
#include "router.hpp"

namespace angzarr {

/**
 * Projection result from a projector handler.
 */
struct Projection {
    std::string key;
    std::string value;
    bool is_delete = false;

    static Projection upsert(const std::string& key, const std::string& value) {
        return {key, value, false};
    }

    static Projection remove(const std::string& key) {
        return {key, "", true};
    }
};

/**
 * Base class for projectors using macro-based handler registration.
 *
 * Usage:
 *   class StockProjector : public Projector {
 *   public:
 *       ANGZARR_PROJECTOR("projector-inventory-stock", "inventory")
 *
 *       ANGZARR_PROJECTS(StockInitialized)
 *       Projection project_StockInitialized(const StockInitialized& event) {
 *           return Projection::upsert(event.sku(), std::to_string(event.quantity()));
 *       }
 *   };
 */
class Projector {
public:
    using ProjectionHandler = std::function<Projection(
        Projector*, const google::protobuf::Any&)>;

    virtual ~Projector() = default;

    /**
     * Get the projector name.
     */
    virtual std::string name() const = 0;

    /**
     * Get the input domain.
     */
    virtual std::string input_domain() const = 0;

    /**
     * Project all events in the book.
     */
    std::vector<Projection> project(const EventBook& book) {
        std::vector<Projection> projections;

        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;

            auto suffix = helpers::type_name_from_url(page.event().type_url());
            auto it = handlers().find(suffix);
            if (it != handlers().end()) {
                projections.push_back(it->second(this, page.event()));
            }
        }
        return projections;
    }

    /**
     * Build a component descriptor.
     */
    Descriptor descriptor() const {
        std::vector<std::string> types;
        for (const auto& [suffix, _] : handlers()) {
            types.push_back(suffix);
        }
        return {name(), component_types::PROJECTOR, {{input_domain(), types}}};
    }

protected:
    /**
     * Register a projection handler.
     */
    static void register_projector_handler(const std::string& suffix, ProjectionHandler handler) {
        handlers()[suffix] = std::move(handler);
    }

private:
    static std::map<std::string, ProjectionHandler>& handlers() {
        static std::map<std::string, ProjectionHandler> h;
        return h;
    }
};

/**
 * Macro to declare a projector.
 */
#define ANGZARR_PROJECTOR(projector_name, in_domain) \
    static constexpr const char* kName = projector_name; \
    static constexpr const char* kInputDomain = in_domain; \
    std::string name() const override { return kName; } \
    std::string input_domain() const override { return kInputDomain; }

} // namespace angzarr
