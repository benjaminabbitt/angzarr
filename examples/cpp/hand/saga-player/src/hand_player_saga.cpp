#include "hand_player_saga.hpp"

#include <iomanip>
#include <sstream>
#include <unordered_map>

namespace hand {
namespace saga {

// Handle PotAwarded: produce DepositFunds commands for each winner.
// Sagas are stateless translators - framework handles sequence stamping.
static std::vector<angzarr::CommandBook> handle_pot_awarded(
    const google::protobuf::Any& event_any, const std::string& source_root,
    const std::string& correlation_id, const std::vector<angzarr::EventBook>& destinations) {
    (void)source_root;
    (void)destinations;  // Sagas are stateless - destinations not used

    examples::PotAwarded event;
    event_any.UnpackTo(&event);

    std::vector<angzarr::CommandBook> commands;

    // Handle all winners
    for (const auto& winner : event.winners()) {
        const std::string& player_root = winner.player_root();

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
        // Framework handles sequence stamping
        page->mutable_header()->mutable_angzarr_deferred();
        page->mutable_command()->CopyFrom(cmd_any);

        commands.push_back(std::move(cmd_book));
    }

    return commands;
}

angzarr::EventRouter create_hand_player_router() {
    return angzarr::EventRouter("saga-hand-player")
        .domain("hand")
        .on("PotAwarded", handle_pot_awarded);
}

}  // namespace saga
}  // namespace hand
