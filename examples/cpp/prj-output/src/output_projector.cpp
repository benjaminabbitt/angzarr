#include "output_projector.hpp"
#include <chrono>
#include <iomanip>
#include <sstream>

namespace projector {

OutputProjector::OutputProjector(OutputFn output_fn, bool show_timestamps)
    : output_fn_(std::move(output_fn))
    , show_timestamps_(show_timestamps) {}

void OutputProjector::set_player_name(const std::string& player_root, const std::string& name) {
    renderer_.set_player_name(player_root, name);
}

void OutputProjector::handle_event(const angzarr::EventPage& event_page) {
    const auto& event_any = event_page.event();
    const std::string& type_url = event_any.type_url();

    std::string text;

    // Player events
    if (type_url.find("PlayerRegistered") != std::string::npos) {
        examples::PlayerRegistered event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_player_registered(event);
        }
    } else if (type_url.find("FundsDeposited") != std::string::npos) {
        examples::FundsDeposited event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_funds_deposited(event);
        }
    } else if (type_url.find("FundsWithdrawn") != std::string::npos) {
        examples::FundsWithdrawn event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_funds_withdrawn(event);
        }
    } else if (type_url.find("FundsReserved") != std::string::npos) {
        examples::FundsReserved event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_funds_reserved(event);
        }
    } else if (type_url.find("FundsReleased") != std::string::npos) {
        examples::FundsReleased event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_funds_released(event);
        }
    }
    // Table events
    else if (type_url.find("TableCreated") != std::string::npos) {
        examples::TableCreated event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_table_created(event);
        }
    } else if (type_url.find("PlayerJoined") != std::string::npos) {
        examples::PlayerJoined event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_player_joined(event);
        }
    } else if (type_url.find("PlayerLeft") != std::string::npos) {
        examples::PlayerLeft event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_player_left(event);
        }
    } else if (type_url.find("HandStarted") != std::string::npos) {
        examples::HandStarted event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_hand_started(event);
        }
    } else if (type_url.find("HandEnded") != std::string::npos) {
        examples::HandEnded event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_hand_ended(event);
        }
    }
    // Hand events
    else if (type_url.find("CardsDealt") != std::string::npos) {
        examples::CardsDealt event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_cards_dealt(event);
        }
    } else if (type_url.find("BlindPosted") != std::string::npos) {
        examples::BlindPosted event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_blind_posted(event);
        }
    } else if (type_url.find("ActionTaken") != std::string::npos) {
        examples::ActionTaken event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_action_taken(event);
        }
    } else if (type_url.find("CommunityCardsDealt") != std::string::npos) {
        examples::CommunityCardsDealt event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_community_cards_dealt(event);
        }
    } else if (type_url.find("PotAwarded") != std::string::npos) {
        examples::PotAwarded event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_pot_awarded(event);
        }
    } else if (type_url.find("HandComplete") != std::string::npos) {
        examples::HandComplete event;
        if (event_any.UnpackTo(&event)) {
            text = renderer_.render_hand_complete(event);
        }
    } else {
        text = "[Unknown event type: " + type_url + "]";
    }

    if (!text.empty()) {
        if (show_timestamps_ && event_page.has_created_at()) {
            auto ts = event_page.created_at();
            auto time_t_val = ts.seconds();
            std::tm tm_val;
            gmtime_r(&time_t_val, &tm_val);

            std::stringstream ss;
            ss << "[" << std::put_time(&tm_val, "%H:%M:%S") << "] " << text;
            output_fn_(ss.str());
        } else {
            output_fn_(text);
        }
    }
}

void OutputProjector::handle_event_book(const angzarr::EventBook& event_book) {
    for (const auto& page : event_book.pages()) {
        handle_event(page);
    }
}

} // namespace projector
