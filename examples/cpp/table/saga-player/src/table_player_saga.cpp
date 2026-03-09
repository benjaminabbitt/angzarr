#include "table_player_saga.hpp"

#include <iomanip>
#include <sstream>
#include <unordered_map>

namespace table {
namespace saga {

// Handle HandEnded: produce ReleaseFunds commands for each player.
// Sagas are stateless translators - framework handles sequence stamping.
static std::vector<angzarr::CommandBook> handle_hand_ended(
    const google::protobuf::Any& event_any, const std::string& source_root,
    const std::string& correlation_id, const std::vector<angzarr::EventBook>& destinations) {
    (void)source_root;
    (void)destinations;  // Sagas are stateless - destinations not used

    examples::HandEnded event;
    event_any.UnpackTo(&event);

    std::vector<angzarr::CommandBook> commands;

    for (const auto& [player_hex, delta] : event.stack_changes()) {
        (void)delta;
        // Convert hex to bytes
        std::string player_root;
        for (size_t i = 0; i < player_hex.length(); i += 2) {
            std::string byte_str = player_hex.substr(i, 2);
            char byte = static_cast<char>(std::stoi(byte_str, nullptr, 16));
            player_root.push_back(byte);
        }

        // Build ReleaseFunds command
        examples::ReleaseFunds release_funds;
        release_funds.set_table_root(event.hand_root());

        google::protobuf::Any cmd_any;
        cmd_any.PackFrom(release_funds, "type.googleapis.com/");

        // Build command book for this player
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

angzarr::EventRouter create_table_player_router() {
    return angzarr::EventRouter("saga-table-player")
        .domain("table")
        .on("HandEnded", handle_hand_ended);
}

}  // namespace saga
}  // namespace table
