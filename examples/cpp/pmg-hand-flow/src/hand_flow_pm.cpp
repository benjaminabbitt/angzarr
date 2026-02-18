// Hand Flow Process Manager - orchestrates poker hand phases across domains.
//
// This PM coordinates the workflow between table and hand domains,
// tracking phase transitions and dispatching commands as the hand progresses.

#include <string>
#include <vector>

#include "angzarr/client.hpp"
#include "angzarr/proto/angzarr/types.pb.h"
#include "angzarr/proto/examples/hand.pb.h"
#include "angzarr/proto/examples/table.pb.h"

using namespace angzarr;
using namespace angzarr::proto::angzarr;
using namespace angzarr::proto::examples;

// docs:start:pm_state
enum class HandPhase { AwaitingDeal, Dealing, Blinds, Betting, Complete };

struct HandFlowState {
    std::string hand_id;
    HandPhase phase = HandPhase::AwaitingDeal;
    int32_t player_count = 0;
};
// docs:end:pm_state

// docs:start:pm_handler
class HandFlowPM : public ProcessManager<HandFlowState> {
public:
    std::vector<CommandBook> handle_hand_started(
        const HandStarted& event, HandFlowState& state) {
        state.hand_id = event.hand_id();
        state.phase = HandPhase::Dealing;
        state.player_count = event.player_count();

        DealCards cmd;
        cmd.set_hand_id(event.hand_id());
        cmd.set_player_count(event.player_count());
        return {build_command("hand", cmd)};
    }

    std::vector<CommandBook> handle_cards_dealt(
        const CardsDealt& event, HandFlowState& state) {
        state.phase = HandPhase::Blinds;

        PostBlinds cmd;
        cmd.set_hand_id(state.hand_id);
        return {build_command("hand", cmd)};
    }

    std::vector<CommandBook> handle_hand_complete(
        const HandComplete& event, HandFlowState& state) {
        state.phase = HandPhase::Complete;

        EndHand cmd;
        cmd.set_hand_id(state.hand_id);
        cmd.set_winner_id(event.winner_id());
        return {build_command("table", cmd)};
    }
};
// docs:end:pm_handler

int main() {
    HandFlowPM pm;
    run_process_manager("pmg-hand-flow", 50393, pm);
    return 0;
}
