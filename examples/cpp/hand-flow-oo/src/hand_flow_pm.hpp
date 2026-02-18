#pragma once

#include <string>
#include <vector>
#include <functional>
#include <map>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/saga.pb.h"
#include "angzarr/process_manager.grpc.pb.h"
#include "angzarr/macros.hpp"
#include "angzarr/errors.hpp"
#include "angzarr/process_manager.hpp"
#include "examples/hand.pb.h"
#include "examples/table.pb.h"

namespace hand_flow_oo {

/// Process manager state for a single hand.
struct PMState {
    std::string hand_root;
    bool hand_in_progress = false;
};

/// Hand Flow Process Manager using OO-style explicit registration.
///
/// This PM orchestrates poker hand flow by:
/// - Tracking when hands start and complete
/// - Coordinating between table and hand domains
class HandFlowPM {
public:
    HandFlowPM() {
        // Register prepare handlers
        prepare_handlers_["HandStarted"] = [this](const google::protobuf::Any& any) {
            examples::HandStarted evt;
            any.UnpackTo(&evt);
            return prepare_HandStarted(evt);
        };

        // Register event handlers
        handlers_["HandStarted"] = [this](const google::protobuf::Any& any, const std::string& corr_id) {
            examples::HandStarted evt;
            any.UnpackTo(&evt);
            return handle_HandStarted(evt, corr_id);
        };
        handlers_["CardsDealt"] = [this](const google::protobuf::Any& any, const std::string& corr_id) {
            examples::CardsDealt evt;
            any.UnpackTo(&evt);
            return handle_CardsDealt(evt, corr_id);
        };
        handlers_["BlindPosted"] = [this](const google::protobuf::Any& any, const std::string& corr_id) {
            examples::BlindPosted evt;
            any.UnpackTo(&evt);
            return handle_BlindPosted(evt, corr_id);
        };
        handlers_["ActionTaken"] = [this](const google::protobuf::Any& any, const std::string& corr_id) {
            examples::ActionTaken evt;
            any.UnpackTo(&evt);
            return handle_ActionTaken(evt, corr_id);
        };
        handlers_["CommunityCardsDealt"] = [this](const google::protobuf::Any& any, const std::string& corr_id) {
            examples::CommunityCardsDealt evt;
            any.UnpackTo(&evt);
            return handle_CommunityCardsDealt(evt, corr_id);
        };
        handlers_["PotAwarded"] = [this](const google::protobuf::Any& any, const std::string& corr_id) {
            examples::PotAwarded evt;
            any.UnpackTo(&evt);
            return handle_PotAwarded(evt, corr_id);
        };

        // Register event appliers
        appliers_["HandStarted"] = [this](PMState& state, const google::protobuf::Any& any) {
            examples::HandStarted evt;
            any.UnpackTo(&evt);
            apply_HandStarted(state, evt);
        };
        appliers_["PotAwarded"] = [this](PMState& state, const google::protobuf::Any& any) {
            examples::PotAwarded evt;
            any.UnpackTo(&evt);
            apply_PotAwarded(state, evt);
        };
    }

    std::string name() const { return "pmg-hand-flow-oo"; }

    std::vector<std::string> input_domains() const {
        return {"table", "hand"};
    }

    /// Build a component descriptor.
    angzarr::Descriptor descriptor() const {
        std::vector<angzarr::TargetDesc> inputs;
        for (const auto& domain : input_domains()) {
            std::vector<std::string> types;
            for (const auto& [suffix, _] : handlers_) {
                types.push_back(suffix);
            }
            inputs.push_back({domain, types});
        }
        return {name(), angzarr::component_types::PROCESS_MANAGER, inputs};
    }

    /// Prepare destinations for events (two-phase protocol).
    std::vector<angzarr::Cover> prepare_destinations(const angzarr::EventBook& book) {
        std::vector<angzarr::Cover> destinations;
        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;
            auto suffix = angzarr::helpers::type_name_from_url(page.event().type_url());
            auto it = prepare_handlers_.find(suffix);
            if (it != prepare_handlers_.end()) {
                auto covers = it->second(page.event());
                destinations.insert(destinations.end(), covers.begin(), covers.end());
            }
        }
        return destinations;
    }

    /// Dispatch events to handlers.
    std::vector<angzarr::CommandBook> dispatch(
            const angzarr::EventBook& book,
            const angzarr::EventBook* prior_events = nullptr,
            const std::vector<angzarr::EventBook>& /* destinations */ = {}) {
        rebuild_state(prior_events);

        auto correlation_id = book.has_cover() ? book.cover().correlation_id() : "";
        if (correlation_id.empty()) {
            return {};  // PMs require correlation ID
        }

        std::vector<angzarr::CommandBook> commands;
        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;

            auto suffix = angzarr::helpers::type_name_from_url(page.event().type_url());

            // Apply event to state first
            auto applier_it = appliers_.find(suffix);
            if (applier_it != appliers_.end()) {
                applier_it->second(state_, page.event());
            }

            // Dispatch to handler
            auto it = handlers_.find(suffix);
            if (it != handlers_.end()) {
                auto cmds = it->second(page.event(), correlation_id);
                commands.insert(commands.end(), cmds.begin(), cmds.end());
            }
        }
        return commands;
    }

    const PMState& state() const { return state_; }

protected:
    /// Declare the hand destination needed when a hand starts.
    std::vector<angzarr::Cover> prepare_HandStarted(const examples::HandStarted& evt) {
        angzarr::Cover cover;
        cover.set_domain("hand");
        cover.mutable_root()->set_value(evt.hand_root());
        return {cover};
    }

    /// Process the HandStarted event from table domain.
    std::vector<angzarr::CommandBook> handle_HandStarted(
            const examples::HandStarted& /* evt */,
            const std::string& /* corr_id */) {
        // No commands to emit - saga-table-hand handles the DealCards command
        return {};
    }

    /// Apply HandStarted to state.
    void apply_HandStarted(PMState& state, const examples::HandStarted& evt) {
        state.hand_root = evt.hand_root();
        state.hand_in_progress = true;
    }

    /// Process the CardsDealt event from hand domain.
    std::vector<angzarr::CommandBook> handle_CardsDealt(
            const examples::CardsDealt& /* evt */,
            const std::string& /* corr_id */) {
        return {};
    }

    /// Process the BlindPosted event from hand domain.
    std::vector<angzarr::CommandBook> handle_BlindPosted(
            const examples::BlindPosted& /* evt */,
            const std::string& /* corr_id */) {
        return {};
    }

    /// Process the ActionTaken event from hand domain.
    std::vector<angzarr::CommandBook> handle_ActionTaken(
            const examples::ActionTaken& /* evt */,
            const std::string& /* corr_id */) {
        return {};
    }

    /// Process the CommunityCardsDealt event from hand domain.
    std::vector<angzarr::CommandBook> handle_CommunityCardsDealt(
            const examples::CommunityCardsDealt& /* evt */,
            const std::string& /* corr_id */) {
        return {};
    }

    /// Process the PotAwarded event from hand domain.
    std::vector<angzarr::CommandBook> handle_PotAwarded(
            const examples::PotAwarded& /* evt */,
            const std::string& /* corr_id */) {
        return {};
    }

    /// Apply PotAwarded to state.
    void apply_PotAwarded(PMState& state, const examples::PotAwarded& /* evt */) {
        state.hand_in_progress = false;
    }

private:
    void rebuild_state(const angzarr::EventBook* event_book) {
        state_ = PMState{};
        if (!event_book) return;

        for (const auto& page : event_book->pages()) {
            if (!page.has_event()) continue;
            auto suffix = angzarr::helpers::type_name_from_url(page.event().type_url());
            auto it = appliers_.find(suffix);
            if (it != appliers_.end()) {
                it->second(state_, page.event());
            }
        }
    }

    using EventHandler = std::function<std::vector<angzarr::CommandBook>(
        const google::protobuf::Any&, const std::string&)>;
    using PrepareHandler = std::function<std::vector<angzarr::Cover>(
        const google::protobuf::Any&)>;
    using EventApplier = std::function<void(PMState&, const google::protobuf::Any&)>;

    std::map<std::string, EventHandler> handlers_;
    std::map<std::string, PrepareHandler> prepare_handlers_;
    std::map<std::string, EventApplier> appliers_;
    PMState state_;
};

} // namespace hand_flow_oo
