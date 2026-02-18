package dev.angzarr.examples.table.sagahand;

import com.google.protobuf.Any;
import dev.angzarr.client.SagaContext;
import dev.angzarr.proto.angzarr.CommandBook;
import dev.angzarr.proto.angzarr.CommandPage;
import dev.angzarr.proto.angzarr.Cover;
import dev.angzarr.proto.angzarr.UUID;
import dev.angzarr.proto.examples.TableSettled;
import dev.angzarr.proto.examples.TransferFunds;

import java.util.ArrayList;
import java.util.List;

/**
 * Saga splitter pattern example for documentation.
 *
 * Demonstrates the splitter pattern where one event triggers commands
 * to multiple different aggregates.
 */

// docs:start:saga_splitter
class SplitterExample {

    List<CommandBook> handleTableSettled(TableSettled event, SagaContext ctx) {
        // Split one event into commands for multiple player aggregates
        List<CommandBook> commands = new ArrayList<>();

        for (var payout : event.getPayoutsList()) {
            var cmd = TransferFunds.newBuilder()
                .setTableRoot(event.getTableRoot())
                .setAmount(payout.getAmount())
                .build();

            long targetSeq = ctx.getSequence("player", payout.getPlayerRoot());

            commands.add(CommandBook.newBuilder()
                .setCover(Cover.newBuilder()
                    .setDomain("player")
                    .setRoot(UUID.newBuilder().setValue(payout.getPlayerRoot()).build())
                    .build())
                .addPages(CommandPage.newBuilder()
                    .setNum((int) targetSeq)
                    .setCommand(Any.pack(cmd))
                    .build())
                .build());
        }

        return commands; // One CommandBook per player
    }
}
// docs:end:saga_splitter
