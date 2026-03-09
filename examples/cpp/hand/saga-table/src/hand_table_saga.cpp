#include "hand_table_saga.hpp"

namespace hand {
namespace saga {

// Handle HandComplete: produce EndHand command for table.
// Sagas are stateless translators - framework handles sequence stamping.
static std::vector<angzarr::CommandBook> handle_hand_complete(
    const google::protobuf::Any& event_any, const std::string& source_root,
    const std::string& correlation_id, const std::vector<angzarr::EventBook>& destinations) {
    (void)destinations;  // Sagas are stateless - destinations not used

    examples::HandComplete event;
    event_any.UnpackTo(&event);

    // Convert PotWinner to PotResult
    examples::EndHand end_hand;
    // hand_root is the source aggregate root (the hand's UUID)
    end_hand.set_hand_root(source_root);

    for (const auto& winner : event.winners()) {
        auto* result = end_hand.add_results();
        result->set_winner_root(winner.player_root());
        result->set_amount(winner.amount());
        result->set_pot_type(winner.pot_type());
    }

    google::protobuf::Any cmd_any;
    cmd_any.PackFrom(end_hand, "type.googleapis.com/");

    angzarr::CommandBook cmd_book;
    cmd_book.mutable_cover()->set_domain("table");
    cmd_book.mutable_cover()->mutable_root()->set_value(event.table_root());
    cmd_book.mutable_cover()->set_correlation_id(correlation_id);

    auto* page = cmd_book.add_pages();
    // Framework handles sequence stamping
    page->mutable_header()->mutable_angzarr_deferred();
    page->mutable_command()->CopyFrom(cmd_any);

    return {std::move(cmd_book)};
}

angzarr::EventRouter create_hand_table_router() {
    return angzarr::EventRouter("saga-hand-table")
        .domain("hand")
        .on("HandComplete", handle_hand_complete);
}

}  // namespace saga
}  // namespace hand
