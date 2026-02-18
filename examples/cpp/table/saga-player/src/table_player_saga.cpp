#include "table_player_saga.hpp"
#include <unordered_map>
#include <sstream>
#include <iomanip>

namespace table {
namespace saga {

angzarr::EventRouter create_table_player_router() {
    return angzarr::EventRouter("saga-table-player", "table")
        .sends("player", "ReleaseFunds")
        .prepare<examples::HandEnded>(prepare_hand_ended)
        .on<examples::HandEnded>(handle_hand_ended);
}

std::vector<angzarr::Cover> prepare_hand_ended(const examples::HandEnded& event) {
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

angzarr::CommandBook handle_hand_ended(
    const examples::HandEnded& event,
    const std::vector<angzarr::EventBook>& destinations) {

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

    // We'll return a single CommandBook with multiple pages for simplicity,
    // but in practice, sagas return multiple CommandBooks (one per destination).
    // For C++, we'll handle the first player only and let the framework call
    // us multiple times, or we can modify to return a vector.

    angzarr::CommandBook cmd_book;

    // For simplicity, handle all players in one go (framework would typically
    // call handler once and expect all commands)
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
} // namespace table
