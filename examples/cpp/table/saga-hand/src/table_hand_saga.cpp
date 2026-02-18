#include "table_hand_saga.hpp"

namespace table {
namespace saga {

angzarr::EventRouter create_table_hand_router() {
    return angzarr::EventRouter("saga-table-hand", "table")
        .sends("hand", "DealCards")
        .prepare<examples::HandStarted>(prepare_hand_started)
        .on<examples::HandStarted>(handle_hand_started);
}

std::vector<angzarr::Cover> prepare_hand_started(const examples::HandStarted& event) {
    std::vector<angzarr::Cover> covers;
    angzarr::Cover cover;
    cover.set_domain("hand");
    cover.mutable_root()->set_value(event.hand_root());
    covers.push_back(cover);
    return covers;
}

angzarr::CommandBook handle_hand_started(
    const examples::HandStarted& event,
    const std::vector<angzarr::EventBook>& destinations) {

    int dest_seq = destinations.empty() ? 0 : destinations[0].next_sequence();

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

    auto* page = cmd_book.add_pages();
    page->set_sequence(dest_seq);
    page->mutable_command()->CopyFrom(cmd_any);

    return cmd_book;
}

} // namespace saga
} // namespace table
