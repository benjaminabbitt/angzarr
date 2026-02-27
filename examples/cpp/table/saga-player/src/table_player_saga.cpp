#include "table_player_saga.hpp"

#include <iomanip>
#include <sstream>
#include <unordered_map>

namespace table {
namespace saga {

// Prepare handler: declare all players from stack_changes as destinations.
static std::vector<angzarr::Cover> prepare_hand_ended(const google::protobuf::Any& event_any,
                                                      const angzarr::UUID* root) {
    (void)root;

    examples::HandEnded event;
    event_any.UnpackTo(&event);

    std::vector<angzarr::Cover> covers;

    for (const auto& [player_hex, delta] : event.stack_changes()) {
        // Convert hex string to bytes
        std::string player_root;
        for (size_t i = 0; i < player_hex.length(); i += 2) {
            std::string byte_str = player_hex.substr(i, 2);
            char byte = static_cast<char>(std::stoi(byte_str, nullptr, 16));
            player_root.push_back(byte);
        }

        angzarr::Cover cover;
        cover.set_domain("player");
        cover.mutable_root()->set_value(player_root);
        covers.push_back(cover);
    }

    return covers;
}

// Handle HandEnded: produce ReleaseFunds commands for each player.
static std::vector<angzarr::CommandBook> handle_hand_ended(
    const google::protobuf::Any& event_any, const std::string& source_root,
    const std::string& correlation_id, const std::vector<angzarr::EventBook>& destinations) {
    (void)source_root;

    examples::HandEnded event;
    event_any.UnpackTo(&event);

    // Build map from player root hex to destination for sequence lookup
    std::unordered_map<std::string, const angzarr::EventBook*> dest_map;
    for (const auto& dest : destinations) {
        if (dest.has_cover() && dest.cover().has_root()) {
            // Convert bytes to hex string
            std::stringstream ss;
            for (unsigned char c : dest.cover().root().value()) {
                ss << std::hex << std::setfill('0') << std::setw(2) << static_cast<int>(c);
            }
            dest_map[ss.str()] = &dest;
        }
    }

    std::vector<angzarr::CommandBook> commands;

    for (const auto& [player_hex, delta] : event.stack_changes()) {
        // Convert hex to bytes
        std::string player_root;
        for (size_t i = 0; i < player_hex.length(); i += 2) {
            std::string byte_str = player_hex.substr(i, 2);
            char byte = static_cast<char>(std::stoi(byte_str, nullptr, 16));
            player_root.push_back(byte);
        }

        // Get sequence from destination
        int dest_seq = 0;
        auto it = dest_map.find(player_hex);
        if (it != dest_map.end()) {
            dest_seq = it->second->next_sequence();
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
        page->set_sequence(dest_seq);
        page->mutable_command()->CopyFrom(cmd_any);

        commands.push_back(std::move(cmd_book));
    }

    return commands;
}

angzarr::EventRouter create_table_player_router() {
    return angzarr::EventRouter("saga-table-player")
        .domain("table")
        .prepare("HandEnded", prepare_hand_ended)
        .on("HandEnded", handle_hand_ended);
}

}  // namespace saga
}  // namespace table
