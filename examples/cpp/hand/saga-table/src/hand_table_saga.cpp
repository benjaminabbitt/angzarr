#include "hand_table_saga.hpp"

namespace hand {
namespace saga {

angzarr::EventRouter create_hand_table_router() {
    return angzarr::EventRouter("saga-hand-table", "hand")
        .sends("table", "EndHand")
        .prepare<examples::HandComplete>(prepare_hand_complete)
        .on<examples::HandComplete>(handle_hand_complete);
}

std::vector<angzarr::Cover> prepare_hand_complete(const examples::HandComplete& event) {
    std::vector<angzarr::Cover> covers;

    angzarr::Cover cover;
    cover.set_domain("table");
    cover.mutable_root()->set_value(event.table_root());
    covers.push_back(cover);

    return covers;
}

angzarr::CommandBook handle_hand_complete(
    const examples::HandComplete& event,
    const std::vector<angzarr::EventBook>& destinations) {

    // Get next sequence from destination state
    int dest_seq = destinations.empty() ? 0 : destinations[0].next_sequence();

    // Convert PotWinner to PotResult
    examples::EndHand end_hand;
    // hand_root comes from the source event - HandComplete should have it
    // but we may need to get it from the request context
    end_hand.set_hand_root("");  // Will be set by framework from source root

    for (const auto& winner : event.winners()) {
        auto* result = end_hand.add_results();
        result->set_winner_root(winner.player_root());
        result->set_amount(winner.amount());
        result->set_pot_type(winner.pot_type());
        // winning_hand is optional
    }

    google::protobuf::Any cmd_any;
    cmd_any.PackFrom(end_hand, "type.googleapis.com/");

    angzarr::CommandBook cmd_book;
    cmd_book.mutable_cover()->set_domain("table");
    cmd_book.mutable_cover()->mutable_root()->set_value(event.table_root());

    auto* page = cmd_book.add_pages();
    page->set_sequence(dest_seq);
    page->mutable_command()->CopyFrom(cmd_any);

    return cmd_book;
}

} // namespace saga
} // namespace hand
