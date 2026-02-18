// Output projector examples for documentation.
//
// This file contains simplified examples used in the projector documentation,
// demonstrating both OO-style and StateRouter patterns.

#include <iostream>
#include <string>
#include <unordered_map>

#include "angzarr/client.hpp"
#include "angzarr/proto/examples/hand.pb.h"
#include "angzarr/proto/examples/player.pb.h"

using namespace angzarr;
using namespace angzarr::proto::examples;

// docs:start:projector_oo
class OutputProjector {
public:
    void handle_player_registered(const PlayerRegistered& event) {
        player_names_[event.player_id()] = event.display_name();
        std::cout << "[Player] " << event.display_name() << " registered\n";
    }

    void handle_funds_deposited(const FundsDeposited& event) {
        auto it = player_names_.find(event.player_id());
        std::string name = (it != player_names_.end()) ? it->second : event.player_id();
        std::cout << "[Player] " << name << " deposited $"
                  << std::fixed << std::setprecision(2)
                  << (event.amount().amount() / 100.0) << "\n";
    }

    void handle_cards_dealt(const CardsDealt& event) {
        for (const auto& player : event.player_cards()) {
            auto it = player_names_.find(player.player_id());
            std::string name = (it != player_names_.end()) ? it->second : player.player_id();
            std::string cards = format_cards(player.hole_cards());
            std::cout << "[Hand] " << name << " dealt " << cards << "\n";
        }
    }

private:
    std::unordered_map<std::string, std::string> player_names_;

    std::string format_cards(const auto& cards) {
        return "cards"; // Simplified for documentation
    }
};
// docs:end:projector_oo

// docs:start:state_router
std::unordered_map<std::string, std::string> player_names;

void handle_player_registered(const PlayerRegistered& event) {
    player_names[event.player_id()] = event.display_name();
    std::cout << "[Player] " << event.display_name() << " registered\n";
}

void handle_funds_deposited(const FundsDeposited& event) {
    auto it = player_names.find(event.player_id());
    std::string name = (it != player_names.end()) ? it->second : event.player_id();
    std::cout << "[Player] " << name << " deposited\n";
}

void handle_cards_dealt(const CardsDealt& event) {
    for (const auto& player : event.player_cards()) {
        auto it = player_names.find(player.player_id());
        std::string name = (it != player_names.end()) ? it->second : player.player_id();
        std::cout << "[Hand] " << name << " dealt cards\n";
    }
}

StateRouter build_router() {
    return StateRouter("prj-output")
        .subscribes("player", {"PlayerRegistered", "FundsDeposited"})
        .subscribes("hand", {"CardsDealt", "ActionTaken", "PotAwarded"})
        .on<PlayerRegistered>(handle_player_registered)
        .on<FundsDeposited>(handle_funds_deposited)
        .on<CardsDealt>(handle_cards_dealt);
}
// docs:end:state_router
