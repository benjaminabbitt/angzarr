// DOC: This file is referenced in docs/docs/examples/sagas.mdx
//      Update documentation when making changes to saga patterns.

#include "table_hand_saga.hpp"

namespace table {
namespace saga {

// docs:start:saga_handler
// Handle HandStarted: produce DealCards command for hand.
// Sagas are stateless translators - framework handles sequence stamping.
static std::vector<angzarr::CommandBook> handle_hand_started(
    const google::protobuf::Any& event_any, const std::string& source_root,
    const std::string& correlation_id, const std::vector<angzarr::EventBook>& destinations) {
    (void)source_root;
    (void)destinations;  // Sagas are stateless - destinations not used

    examples::HandStarted event;
    event_any.UnpackTo(&event);

    // Build DealCards command from HandStarted event
    examples::DealCards deal_cards;
    deal_cards.set_table_root(event.hand_root());
    deal_cards.set_hand_number(event.hand_number());
    deal_cards.set_game_variant(event.game_variant());
    deal_cards.set_dealer_position(event.dealer_position());
    deal_cards.set_small_blind(event.small_blind());
    deal_cards.set_big_blind(event.big_blind());

    // Add players from active players
    for (const auto& seat : event.active_players()) {
        auto* player = deal_cards.add_players();
        player->set_player_root(seat.player_root());
        player->set_position(seat.position());
        player->set_stack(seat.stack());
    }

    // Pack command
    google::protobuf::Any cmd_any;
    cmd_any.PackFrom(deal_cards, "type.googleapis.com/");

    // Build command book
    angzarr::CommandBook cmd_book;
    cmd_book.mutable_cover()->set_domain("hand");
    cmd_book.mutable_cover()->mutable_root()->set_value(event.hand_root());
    cmd_book.mutable_cover()->set_correlation_id(correlation_id);

    auto* page = cmd_book.add_pages();
    // Framework handles sequence stamping
    page->mutable_header()->mutable_angzarr_deferred();
    page->mutable_command()->CopyFrom(cmd_any);

    return {std::move(cmd_book)};
}
// docs:end:saga_handler

// docs:start:event_router
angzarr::EventRouter create_table_hand_router() {
    return angzarr::EventRouter("saga-table-hand")
        .domain("table")
        .on("HandStarted", handle_hand_started);
}
// docs:end:event_router

}  // namespace saga
}  // namespace table
