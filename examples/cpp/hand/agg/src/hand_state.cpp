#include "hand_state.hpp"
#include <algorithm>
#include <random>
#include <sstream>
#include <iomanip>

namespace hand {

int64_t HandState::get_pot_total() const {
    int64_t total = 0;
    for (const auto& pot : pots) {
        total += pot.amount;
    }
    return total;
}

const PlayerHandInfo* HandState::get_player(const std::string& player_root) const {
    for (const auto& [pos, player] : players) {
        if (player.player_root == player_root) {
            return &player;
        }
    }
    return nullptr;
}

PlayerHandInfo* HandState::get_player_mut(const std::string& player_root) {
    for (auto& [pos, player] : players) {
        if (player.player_root == player_root) {
            return &player;
        }
    }
    return nullptr;
}

std::vector<const PlayerHandInfo*> HandState::get_active_players() const {
    std::vector<const PlayerHandInfo*> result;
    for (const auto& [pos, player] : players) {
        if (!player.has_folded && !player.is_all_in) {
            result.push_back(&player);
        }
    }
    return result;
}

std::vector<const PlayerHandInfo*> HandState::get_players_in_hand() const {
    std::vector<const PlayerHandInfo*> result;
    for (const auto& [pos, player] : players) {
        if (!player.has_folded) {
            result.push_back(&player);
        }
    }
    return result;
}

HandState HandState::from_event_book(const angzarr::EventBook& event_book) {
    HandState state;
    for (const auto& page : event_book.pages()) {
        apply_event(state, page.event());
    }
    return state;
}

void HandState::apply_event(HandState& state, const google::protobuf::Any& event_any) {
    const std::string& type_url = event_any.type_url();

    if (type_url.find("CardsDealt") != std::string::npos) {
        examples::CardsDealt event;
        if (event_any.UnpackTo(&event)) {
            // Build hand_id from table_root hex + hand_number
            std::stringstream ss;
            for (unsigned char c : event.table_root()) {
                ss << std::hex << std::setfill('0') << std::setw(2) << static_cast<int>(c);
            }
            state.hand_id = ss.str() + "_" + std::to_string(event.hand_number());
            state.table_root = event.table_root();
            state.hand_number = event.hand_number();
            state.game_variant = event.game_variant();
            state.dealer_position = event.dealer_position();
            state.status = "betting";
            state.current_phase = examples::PREFLOP;

            // Initialize players
            for (const auto& player : event.players()) {
                PlayerHandInfo info;
                info.player_root = player.player_root();
                info.position = player.position();
                info.stack = player.stack();
                state.players[player.position()] = info;
            }

            // Track dealt cards
            std::set<std::pair<int, int>> dealt_cards;
            for (const auto& pc : event.player_cards()) {
                auto* player = state.get_player_mut(pc.player_root());
                if (player) {
                    for (const auto& card : pc.cards()) {
                        Card c{card.suit(), card.rank()};
                        player->hole_cards.push_back(c);
                        dealt_cards.insert({static_cast<int>(card.suit()), card.rank()});
                    }
                }
            }

            // Build remaining deck
            state.remaining_deck.clear();
            std::vector<examples::Suit> suits = {
                examples::CLUBS, examples::DIAMONDS,
                examples::HEARTS, examples::SPADES
            };
            for (auto suit : suits) {
                for (int rank = 2; rank <= 14; ++rank) {
                    if (dealt_cards.find({static_cast<int>(suit), rank}) == dealt_cards.end()) {
                        state.remaining_deck.push_back(Card{suit, rank});
                    }
                }
            }
            // Shuffle remaining deck
            std::random_device rd;
            std::mt19937 g(rd());
            std::shuffle(state.remaining_deck.begin(), state.remaining_deck.end(), g);

            // Initialize main pot
            PotInfo main_pot;
            main_pot.amount = 0;
            main_pot.pot_type = "main";
            for (const auto& [pos, player] : state.players) {
                main_pot.eligible_players.push_back(player.player_root);
            }
            state.pots = {main_pot};
        }
    }
    else if (type_url.find("BlindPosted") != std::string::npos) {
        examples::BlindPosted event;
        if (event_any.UnpackTo(&event)) {
            auto* player = state.get_player_mut(event.player_root());
            if (player) {
                player->stack = event.player_stack();
                player->bet_this_round = event.amount();
                player->total_invested += event.amount();
                if (event.blind_type() == "small") {
                    state.small_blind_position = player->position;
                    state.small_blind = event.amount();
                } else if (event.blind_type() == "big") {
                    state.big_blind_position = player->position;
                    state.big_blind = event.amount();
                    state.current_bet = event.amount();
                    state.min_raise = event.amount();
                }
            }
            if (!state.pots.empty()) {
                state.pots[0].amount = event.pot_total();
            }
            state.status = "betting";
        }
    }
    else if (type_url.find("ActionTaken") != std::string::npos) {
        examples::ActionTaken event;
        if (event_any.UnpackTo(&event)) {
            auto* player = state.get_player_mut(event.player_root());
            if (player) {
                player->stack = event.player_stack();
                player->has_acted = true;
                if (event.action() == examples::FOLD) {
                    player->has_folded = true;
                } else if (event.action() == examples::CALL ||
                           event.action() == examples::BET ||
                           event.action() == examples::RAISE) {
                    player->bet_this_round += event.amount();
                    player->total_invested += event.amount();
                } else if (event.action() == examples::ALL_IN) {
                    player->is_all_in = true;
                    player->bet_this_round += event.amount();
                    player->total_invested += event.amount();
                }
                if (event.action() == examples::BET ||
                    event.action() == examples::RAISE ||
                    event.action() == examples::ALL_IN) {
                    if (player->bet_this_round > state.current_bet) {
                        int64_t raise_amount = player->bet_this_round - state.current_bet;
                        state.current_bet = player->bet_this_round;
                        state.min_raise = std::max(state.min_raise, raise_amount);
                    }
                }
            }
            if (!state.pots.empty()) {
                state.pots[0].amount = event.pot_total();
            }
            state.action_on_position = -1;
        }
    }
    else if (type_url.find("CommunityCardsDealt") != std::string::npos) {
        examples::CommunityCardsDealt event;
        if (event_any.UnpackTo(&event)) {
            for (const auto& card : event.cards()) {
                Card c{card.suit(), card.rank()};
                state.community_cards.push_back(c);
                // Remove from remaining deck
                auto it = std::find(state.remaining_deck.begin(),
                                    state.remaining_deck.end(), c);
                if (it != state.remaining_deck.end()) {
                    state.remaining_deck.erase(it);
                }
            }
            state.current_phase = event.phase();
            state.status = "betting";
            // Reset betting round
            for (auto& [pos, player] : state.players) {
                player.bet_this_round = 0;
                player.has_acted = false;
            }
            state.current_bet = 0;
        }
    }
    else if (type_url.find("ShowdownStarted") != std::string::npos) {
        state.status = "showdown";
    }
    else if (type_url.find("PotAwarded") != std::string::npos) {
        examples::PotAwarded event;
        if (event_any.UnpackTo(&event)) {
            for (const auto& winner : event.winners()) {
                auto* player = state.get_player_mut(winner.player_root());
                if (player) {
                    player->stack += winner.amount();
                }
            }
        }
    }
    else if (type_url.find("HandComplete") != std::string::npos) {
        state.status = "complete";
    }
}

} // namespace hand
