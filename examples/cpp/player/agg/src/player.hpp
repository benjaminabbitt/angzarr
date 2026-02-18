#pragma once

#include "player_state.hpp"
#include "angzarr/aggregate.hpp"
#include "examples/player.pb.h"

namespace player {

/// Player aggregate - OO style using CRTP pattern.
class Player : public angzarr::Aggregate<Player, PlayerState> {
public:
    static constexpr const char* DOMAIN = "player";

    Player();

    /// Apply a single event to state (called by base class).
    void apply_event(PlayerState& state, const google::protobuf::Any& event_any) {
        PlayerState::apply_event(state, event_any);
    }

    // State accessors
    bool exists() const { return state_.exists(); }
    const std::string& player_id() const { return state_.player_id; }
    const std::string& display_name() const { return state_.display_name; }
    const std::string& email() const { return state_.email; }
    examples::PlayerType player_type() const { return state_.player_type; }
    const std::string& ai_model_id() const { return state_.ai_model_id; }
    int64_t bankroll() const { return state_.bankroll; }
    int64_t reserved_funds() const { return state_.reserved_funds; }
    int64_t available_balance() const { return state_.available_balance(); }
    bool is_ai() const { return state_.is_ai(); }

    // Command handlers
    examples::PlayerRegistered handle_register(const examples::RegisterPlayer& cmd);
    examples::FundsDeposited handle_deposit(const examples::DepositFunds& cmd);
    examples::FundsWithdrawn handle_withdraw(const examples::WithdrawFunds& cmd);
    examples::FundsReserved handle_reserve(const examples::ReserveFunds& cmd);
    examples::FundsReleased handle_release(const examples::ReleaseFunds& cmd);
    examples::FundsTransferred handle_transfer(const examples::TransferFunds& cmd);
};

} // namespace player
