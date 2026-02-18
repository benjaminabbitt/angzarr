#include "hand_process.hpp"
#include <algorithm>
#include <sstream>
#include <iomanip>

namespace hand_flow {

HandProcessManager::HandProcessManager(CommandSender command_sender)
    : command_sender_(std::move(command_sender)) {}

HandProcess* HandProcessManager::get_process(const std::string& hand_id) {
    auto it = processes_.find(hand_id);
    return it != processes_.end() ? &it->second : nullptr;
}

std::optional<angzarr::CommandBook> HandProcessManager::start_hand(
    const examples::HandStarted& event) {

    // Build hand_id from table_root hex + hand_number
    std::stringstream ss;
    for (unsigned char c : event.hand_root()) {
        ss << std::hex << std::setfill('0') << std::setw(2) << static_cast<int>(c);
    }
    std::string hand_id = ss.str() + "_" + std::to_string(event.hand_number());

    HandProcess process;
    process.hand_id = hand_id;
    process.hand_number = event.hand_number();
    process.game_variant = event.game_variant();
    process.dealer_position = event.dealer_position();
    process.small_blind_position = event.small_blind_position();
    process.big_blind_position = event.big_blind_position();
    process.small_blind = event.small_blind();
    process.big_blind = event.big_blind();
    process.phase = HandPhase::DEALING;

    // Initialize player states
    for (const auto& player : event.active_players()) {
        PlayerState ps;
        ps.player_root = player.player_root();
        ps.position = player.position();
        ps.stack = player.stack();
        process.players[player.position()] = ps;
        process.active_positions.push_back(player.position());
    }

    std::sort(process.active_positions.begin(), process.active_positions.end());
    processes_[hand_id] = std::move(process);

    // No command issued yet - wait for CardsDealt
    return std::nullopt;
}

std::optional<angzarr::CommandBook> HandProcessManager::handle_cards_dealt(
    const examples::CardsDealt& event) {

    std::stringstream ss;
    for (unsigned char c : event.table_root()) {
        ss << std::hex << std::setfill('0') << std::setw(2) << static_cast<int>(c);
    }
    std::string hand_id = ss.str() + "_" + std::to_string(event.hand_number());

    auto* process = get_process(hand_id);
    if (!process) {
        return std::nullopt;
    }

    process->phase = HandPhase::POSTING_BLINDS;
    process->min_raise = process->big_blind;

    return post_next_blind(*process);
}

std::optional<angzarr::CommandBook> HandProcessManager::handle_blind_posted(
    const examples::BlindPosted& event) {

    // Find the process by player_root (simplified - would need hand_id in real impl)
    for (auto& [hand_id, process] : processes_) {
        if (process.phase != HandPhase::POSTING_BLINDS) {
            continue;
        }

        // Update player state
        for (auto& [pos, player] : process.players) {
            if (player.player_root == event.player_root()) {
                player.stack = event.player_stack();
                player.bet_this_round = event.amount();
                player.total_invested = event.amount();
                break;
            }
        }

        process.pot_total = event.pot_total();

        if (event.blind_type() == "small") {
            process.small_blind_posted = true;
            process.current_bet = event.amount();
            return post_next_blind(process);
        } else if (event.blind_type() == "big") {
            process.big_blind_posted = true;
            process.current_bet = event.amount();
            start_betting(process);
            return std::nullopt;  // Betting starts, waiting for player action
        }
    }

    return std::nullopt;
}

std::optional<angzarr::CommandBook> HandProcessManager::handle_action_taken(
    const examples::ActionTaken& event) {

    // Find the process
    for (auto& [hand_id, process] : processes_) {
        if (process.phase != HandPhase::BETTING) {
            continue;
        }

        // Update player state
        for (auto& [pos, player] : process.players) {
            if (player.player_root == event.player_root()) {
                player.stack = event.player_stack();
                player.has_acted = true;

                if (event.action() == examples::FOLD) {
                    player.has_folded = true;
                } else if (event.action() == examples::ALL_IN) {
                    player.is_all_in = true;
                    player.bet_this_round += event.amount();
                    player.total_invested += event.amount();
                } else if (event.action() == examples::CALL ||
                           event.action() == examples::BET ||
                           event.action() == examples::RAISE) {
                    player.bet_this_round += event.amount();
                    player.total_invested += event.amount();
                }

                if (event.action() == examples::BET ||
                    event.action() == examples::RAISE ||
                    event.action() == examples::ALL_IN) {
                    if (player.bet_this_round > process.current_bet) {
                        int64_t raise_amount = player.bet_this_round - process.current_bet;
                        process.current_bet = player.bet_this_round;
                        process.min_raise = std::max(process.min_raise, raise_amount);
                        process.last_aggressor = pos;
                        // Reset has_acted for other active players
                        for (auto& [p_pos, p] : process.players) {
                            if (p_pos != pos && !p.has_folded && !p.is_all_in) {
                                p.has_acted = false;
                            }
                        }
                    }
                }
                break;
            }
        }

        process.pot_total = event.pot_total();

        if (is_betting_complete(process)) {
            return end_betting_round(process);
        } else {
            advance_action(process);
            return std::nullopt;  // Waiting for next action
        }
    }

    return std::nullopt;
}

std::optional<angzarr::CommandBook> HandProcessManager::handle_community_cards_dealt(
    const examples::CommunityCardsDealt& event) {

    for (auto& [hand_id, process] : processes_) {
        if (process.phase == HandPhase::DEALING_COMMUNITY) {
            process.betting_phase = event.phase();
            start_betting(process);
            return std::nullopt;
        }
    }
    return std::nullopt;
}

std::optional<angzarr::CommandBook> HandProcessManager::handle_showdown_started(
    const examples::ShowdownStarted& event) {

    // In a real implementation, would wait for reveals then award pot
    for (auto& [hand_id, process] : processes_) {
        if (process.phase == HandPhase::SHOWDOWN) {
            return build_award_pot_cmd(process);
        }
    }
    return std::nullopt;
}

void HandProcessManager::handle_pot_awarded(const examples::PotAwarded& event) {
    for (auto& [hand_id, process] : processes_) {
        if (process.phase != HandPhase::COMPLETE) {
            process.phase = HandPhase::COMPLETE;
        }
    }
}

std::optional<angzarr::CommandBook> HandProcessManager::post_next_blind(HandProcess& process) {
    if (!process.small_blind_posted) {
        auto it = process.players.find(process.small_blind_position);
        if (it != process.players.end()) {
            return build_post_blind_cmd(process, it->second, "small", process.small_blind);
        }
    } else if (!process.big_blind_posted) {
        auto it = process.players.find(process.big_blind_position);
        if (it != process.players.end()) {
            return build_post_blind_cmd(process, it->second, "big", process.big_blind);
        }
    }
    return std::nullopt;
}

angzarr::CommandBook HandProcessManager::build_post_blind_cmd(
    const HandProcess& process,
    const PlayerState& player,
    const std::string& blind_type,
    int64_t amount) {

    examples::PostBlind post_blind;
    post_blind.set_player_root(player.player_root);
    post_blind.set_blind_type(blind_type);
    post_blind.set_amount(amount);

    google::protobuf::Any cmd_any;
    cmd_any.PackFrom(post_blind, "type.googleapis.com/");

    // Extract hand_root from hand_id
    std::string hand_root_hex = process.hand_id.substr(0, process.hand_id.find('_'));
    std::string hand_root;
    for (size_t i = 0; i < hand_root_hex.length(); i += 2) {
        std::string byte_str = hand_root_hex.substr(i, 2);
        char byte = static_cast<char>(std::stoi(byte_str, nullptr, 16));
        hand_root.push_back(byte);
    }

    angzarr::CommandBook cmd_book;
    cmd_book.mutable_cover()->set_domain("hand");
    cmd_book.mutable_cover()->mutable_root()->set_value(hand_root);

    auto* page = cmd_book.add_pages();
    page->mutable_command()->CopyFrom(cmd_any);

    return cmd_book;
}

void HandProcessManager::start_betting(HandProcess& process) {
    process.phase = HandPhase::BETTING;

    // Reset betting state
    for (auto& [pos, player] : process.players) {
        player.bet_this_round = 0;
        player.has_acted = false;
    }
    process.current_bet = 0;

    // Determine first to act
    if (process.betting_phase == examples::PREFLOP) {
        process.action_on = find_next_active(process, process.big_blind_position);
    } else {
        process.action_on = find_next_active(process, process.dealer_position);
    }
}

void HandProcessManager::advance_action(HandProcess& process) {
    process.action_on = find_next_active(process, process.action_on);
}

int HandProcessManager::find_next_active(const HandProcess& process, int after_position) const {
    const auto& positions = process.active_positions;
    if (positions.empty()) {
        return -1;
    }

    // Find starting index
    size_t start_idx = 0;
    for (size_t i = 0; i < positions.size(); ++i) {
        if (positions[i] > after_position) {
            start_idx = i;
            break;
        }
    }

    // Find next active player
    for (size_t i = 0; i < positions.size(); ++i) {
        size_t idx = (start_idx + i) % positions.size();
        int pos = positions[idx];
        auto it = process.players.find(pos);
        if (it != process.players.end() && !it->second.has_folded && !it->second.is_all_in) {
            return pos;
        }
    }

    return -1;
}

bool HandProcessManager::is_betting_complete(const HandProcess& process) const {
    int active_count = 0;
    for (const auto& [pos, player] : process.players) {
        if (!player.has_folded && !player.is_all_in) {
            ++active_count;
            if (!player.has_acted) {
                return false;
            }
            if (player.bet_this_round < process.current_bet) {
                return false;
            }
        }
    }
    return active_count <= 1 || active_count > 0;
}

std::optional<angzarr::CommandBook> HandProcessManager::end_betting_round(HandProcess& process) {
    // Count players in hand
    std::vector<const PlayerState*> players_in_hand;
    for (const auto& [pos, player] : process.players) {
        if (!player.has_folded) {
            players_in_hand.push_back(&player);
        }
    }

    // If only one player left, award pot
    if (players_in_hand.size() == 1) {
        process.phase = HandPhase::COMPLETE;
        return build_award_pot_cmd(process);
    }

    // Advance to next phase
    if (process.game_variant == examples::TEXAS_HOLDEM ||
        process.game_variant == examples::OMAHA) {
        if (process.betting_phase == examples::PREFLOP) {
            process.phase = HandPhase::DEALING_COMMUNITY;
            return build_deal_community_cmd(process, 3);
        } else if (process.betting_phase == examples::FLOP) {
            process.phase = HandPhase::DEALING_COMMUNITY;
            return build_deal_community_cmd(process, 1);
        } else if (process.betting_phase == examples::TURN) {
            process.phase = HandPhase::DEALING_COMMUNITY;
            return build_deal_community_cmd(process, 1);
        } else if (process.betting_phase == examples::RIVER) {
            process.phase = HandPhase::SHOWDOWN;
            return build_award_pot_cmd(process);
        }
    }

    return std::nullopt;
}

angzarr::CommandBook HandProcessManager::build_deal_community_cmd(
    const HandProcess& process, int count) {

    examples::DealCommunityCards deal;
    deal.set_count(count);

    google::protobuf::Any cmd_any;
    cmd_any.PackFrom(deal, "type.googleapis.com/");

    // Extract hand_root
    std::string hand_root_hex = process.hand_id.substr(0, process.hand_id.find('_'));
    std::string hand_root;
    for (size_t i = 0; i < hand_root_hex.length(); i += 2) {
        std::string byte_str = hand_root_hex.substr(i, 2);
        char byte = static_cast<char>(std::stoi(byte_str, nullptr, 16));
        hand_root.push_back(byte);
    }

    angzarr::CommandBook cmd_book;
    cmd_book.mutable_cover()->set_domain("hand");
    cmd_book.mutable_cover()->mutable_root()->set_value(hand_root);

    auto* page = cmd_book.add_pages();
    page->mutable_command()->CopyFrom(cmd_any);

    return cmd_book;
}

angzarr::CommandBook HandProcessManager::build_award_pot_cmd(const HandProcess& process) {
    // Collect players in hand
    std::vector<const PlayerState*> players_in_hand;
    for (const auto& [pos, player] : process.players) {
        if (!player.has_folded) {
            players_in_hand.push_back(&player);
        }
    }

    examples::AwardPot award;
    if (!players_in_hand.empty()) {
        // Simple split (real impl would evaluate hands)
        int64_t split = process.pot_total / static_cast<int64_t>(players_in_hand.size());
        int64_t remainder = process.pot_total % static_cast<int64_t>(players_in_hand.size());

        for (size_t i = 0; i < players_in_hand.size(); ++i) {
            auto* pot_award = award.add_awards();
            pot_award->set_player_root(players_in_hand[i]->player_root);
            pot_award->set_amount(split + (static_cast<int64_t>(i) < remainder ? 1 : 0));
            pot_award->set_pot_type("main");
        }
    }

    google::protobuf::Any cmd_any;
    cmd_any.PackFrom(award, "type.googleapis.com/");

    // Extract hand_root
    std::string hand_root_hex = process.hand_id.substr(0, process.hand_id.find('_'));
    std::string hand_root;
    for (size_t i = 0; i < hand_root_hex.length(); i += 2) {
        std::string byte_str = hand_root_hex.substr(i, 2);
        char byte = static_cast<char>(std::stoi(byte_str, nullptr, 16));
        hand_root.push_back(byte);
    }

    angzarr::CommandBook cmd_book;
    cmd_book.mutable_cover()->set_domain("hand");
    cmd_book.mutable_cover()->mutable_root()->set_value(hand_root);

    auto* page = cmd_book.add_pages();
    page->mutable_command()->CopyFrom(cmd_any);

    return cmd_book;
}

} // namespace hand_flow
