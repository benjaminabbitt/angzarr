#include "hand_player_saga.hpp"

#include <iomanip>
#include <sstream>
#include <unordered_map>

namespace hand {
namespace saga {

// Prepare handler: declare all winners as destinations.
static std::vector<angzarr::Cover> prepare_pot_awarded(const google::protobuf::Any& event_any,
                                                       const angzarr::UUID* root) {
    (void)root;

    examples::PotAwarded event;
    event_any.UnpackTo(&event);

    std::vector<angzarr::Cover> covers;
    for (const auto& winner : event.winners()) {
        angzarr::Cover cover;
        cover.set_domain("player");
        cover.mutable_root()->set_value(winner.player_root());
        covers.push_back(cover);
    }

    return covers;
}

// Handle PotAwarded: produce DepositFunds commands for each winner.
static std::vector<angzarr::CommandBook> handle_pot_awarded(
    const google::protobuf::Any& event_any, const std::string& source_root,
    const std::string& correlation_id, const std::vector<angzarr::EventBook>& destinations) {
    (void)source_root;

    examples::PotAwarded event;
    event_any.UnpackTo(&event);

    // Build map from player root to destination for sequence lookup
    std::unordered_map<std::string, const angzarr::EventBook*> dest_map;
    for (const auto& dest : destinations) {
        if (dest.has_cover() && dest.cover().has_root()) {
            dest_map[dest.cover().root().value()] = &dest;
        }
    }

    std::vector<angzarr::CommandBook> commands;

    // Handle all winners
    for (const auto& winner : event.winners()) {
        const std::string& player_root = winner.player_root();

        // Get sequence from destination
        int dest_seq = 0;
        auto it = dest_map.find(player_root);
        if (it != dest_map.end()) {
            dest_seq = it->second->next_sequence();
        }

        // Build DepositFunds command
        examples::DepositFunds deposit_funds;
        deposit_funds.mutable_amount()->set_amount(winner.amount());

        google::protobuf::Any cmd_any;
        cmd_any.PackFrom(deposit_funds, "type.googleapis.com/");

        // Build command book
        angzarr::CommandBook cmd_book;
        cmd_book.mutable_cover()->set_domain("player");
        cmd_book.mutable_cover()->mutable_root()->set_value(player_root);
        cmd_book.mutable_cover()->set_correlation_id(correlation_id);

        auto* page = cmd_book.add_pages();
        page->set_sequence(dest_seq);
        page->mutable_command()->CopyFrom(cmd_any);

        commands.push_back(std::move(cmd_book));
    }

    return commands;
}

angzarr::EventRouter create_hand_player_router() {
    return angzarr::EventRouter("saga-hand-player")
        .domain("hand")
        .prepare("PotAwarded", prepare_pot_awarded)
        .on("PotAwarded", handle_pot_awarded);
}

}  // namespace saga
}  // namespace hand
