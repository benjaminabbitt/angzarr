// Saga splitter pattern example for documentation.
//
// Demonstrates the splitter pattern where one event triggers commands
// to multiple different aggregates.

#include <vector>

#include "angzarr/client.hpp"
#include "angzarr/proto/angzarr/types.pb.h"
#include "angzarr/proto/examples/table.pb.h"
#include "angzarr/proto/examples/player.pb.h"

using namespace angzarr;
using namespace angzarr::proto::angzarr;
using namespace angzarr::proto::examples;

// docs:start:saga_splitter
std::vector<CommandBook> handle_table_settled(
    const TableSettled& event, const SagaContext& ctx) {
    // Split one event into commands for multiple player aggregates
    std::vector<CommandBook> commands;

    for (const auto& payout : event.payouts()) {
        TransferFunds cmd;
        cmd.set_table_root(event.table_root());
        *cmd.mutable_amount() = payout.amount();

        uint32_t target_seq = ctx.get_sequence("player", payout.player_root());

        CommandBook book;
        auto* cover = book.mutable_cover();
        cover->set_domain("player");
        cover->mutable_root()->set_value(payout.player_root());

        auto* page = book.add_pages();
        page->set_num(target_seq);
        page->mutable_command()->PackFrom(cmd);

        commands.push_back(book);
    }

    return commands; // One CommandBook per player
}
// docs:end:saga_splitter
