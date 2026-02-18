#include "hand_player_saga.hpp"
#include <unordered_map>
#include <sstream>
#include <iomanip>

namespace hand {
namespace saga {

angzarr::EventRouter create_hand_player_router() {
    return angzarr::EventRouter("saga-hand-player", "hand")
        .sends("player", "DepositFunds")
        .prepare<examples::PotAwarded>(prepare_pot_awarded)
        .on<examples::PotAwarded>(handle_pot_awarded);
}

std::vector<angzarr::Cover> prepare_pot_awarded(const examples::PotAwarded& event) {
    std::vector<angzarr::Cover> covers;

    for (const auto& winner : event.winners()) {
        angzarr::Cover cover;
        cover.set_domain("player");
        cover.mutable_root()->set_value(winner.player_root());
        covers.push_back(cover);
    }

    return covers;
}

angzarr::CommandBook handle_pot_awarded(
    const examples::PotAwarded& event,
    const std::vector<angzarr::EventBook>& destinations) {

    // Build map from player root to destination for sequence lookup
    std::unordered_map<std::string, const angzarr::EventBook*> dest_map;
    for (const auto& dest : destinations) {
        if (dest.has_cover() && dest.cover().has_root()) {
            // Use raw bytes as key
            dest_map[dest.cover().root().value()] = &dest;
        }
    }

    angzarr::CommandBook cmd_book;

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

        // Set cover for this command book
        cmd_book.mutable_cover()->set_domain("player");
        cmd_book.mutable_cover()->mutable_root()->set_value(player_root);

        auto* page = cmd_book.add_pages();
        page->set_sequence(dest_seq);
        page->mutable_command()->CopyFrom(cmd_any);
    }

    return cmd_book;
}

} // namespace saga
} // namespace hand
