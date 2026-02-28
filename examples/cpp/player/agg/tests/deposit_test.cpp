#include <gtest/gtest.h>

#include "../handlers/deposit_handler.hpp"
#include "../src/player_state.hpp"
#include "angzarr/errors.hpp"
#include "examples/player.pb.h"
#include "examples/poker_types.pb.h"

namespace player {
namespace handlers {

// docs:start:unit_test_deposit
TEST(DepositHandlerTest, DepositIncreasesBankroll) {
    PlayerState state;
    state.player_id = "player_1";
    state.bankroll = 1000;

    examples::DepositFunds cmd;
    cmd.mutable_amount()->set_amount(500);
    cmd.mutable_amount()->set_currency_code("CHIPS");

    auto event = compute(cmd, state, 500);

    EXPECT_EQ(event.new_balance().amount(), 1500);
}

TEST(DepositHandlerTest, RejectsNonExistentPlayer) {
    PlayerState state;  // player_id empty = doesn't exist

    EXPECT_THROW(
        {
            try {
                guard(state);
            } catch (const angzarr::CommandRejectedError& e) {
                EXPECT_NE(std::string(e.what()).find("does not exist"), std::string::npos);
                throw;
            }
        },
        angzarr::CommandRejectedError);
}

TEST(DepositHandlerTest, RejectsZeroAmount) {
    examples::DepositFunds cmd;
    cmd.mutable_amount()->set_amount(0);
    cmd.mutable_amount()->set_currency_code("CHIPS");

    EXPECT_THROW(
        {
            try {
                validate(cmd);
            } catch (const angzarr::CommandRejectedError& e) {
                EXPECT_NE(std::string(e.what()).find("positive"), std::string::npos);
                throw;
            }
        },
        angzarr::CommandRejectedError);
}
// docs:end:unit_test_deposit

}  // namespace handlers
}  // namespace player
