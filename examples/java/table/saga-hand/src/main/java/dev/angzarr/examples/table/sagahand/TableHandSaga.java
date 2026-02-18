// DOC: This file is referenced in docs/docs/examples/sagas.mdx
//      Update documentation when making changes to saga patterns.
package dev.angzarr.examples.table.sagahand;

import com.google.protobuf.Any;
import dev.angzarr.*;
import dev.angzarr.client.Saga;
import dev.angzarr.client.annotations.Prepares;
import dev.angzarr.client.annotations.ReactsTo;
import dev.angzarr.examples.*;

import java.util.ArrayList;
import java.util.List;

/**
 * Saga: Table -> Hand (OO Pattern)
 *
 * <p>Reacts to HandStarted events from Table domain.
 * Sends DealCards commands to Hand domain.
 */
public class TableHandSaga extends Saga {

    @Override
    public String getName() {
        return "saga-table-hand";
    }

    @Override
    public String getInputDomain() {
        return "table";
    }

    @Override
    public String getOutputDomain() {
        return "hand";
    }

    @Prepares(HandStarted.class)
    public List<Cover> prepareHandStarted(HandStarted event) {
        return List.of(
            Cover.newBuilder()
                .setDomain("hand")
                .setRoot(UUID.newBuilder().setValue(event.getHandRoot()))
                .build()
        );
    }

    // docs:start:saga_handler
    @ReactsTo(HandStarted.class)
    public CommandBook handleHandStarted(HandStarted event, List<EventBook> destinations) {
        int destSeq = Saga.nextSequence(destinations.isEmpty() ? null : destinations.get(0));

        // Convert SeatSnapshot to PlayerInHand
        List<PlayerInHand> players = new ArrayList<>();
        for (SeatSnapshot seat : event.getActivePlayersList()) {
            players.add(PlayerInHand.newBuilder()
                .setPlayerRoot(seat.getPlayerRoot())
                .setPosition(seat.getPosition())
                .setStack(seat.getStack())
                .build());
        }

        // Build DealCards command
        DealCards dealCards = DealCards.newBuilder()
            .setTableRoot(event.getHandRoot())
            .setHandNumber(event.getHandNumber())
            .setGameVariant(event.getGameVariant())
            .setDealerPosition(event.getDealerPosition())
            .setSmallBlind(event.getSmallBlind())
            .setBigBlind(event.getBigBlind())
            .addAllPlayers(players)
            .build();

        return CommandBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain("hand")
                .setRoot(UUID.newBuilder().setValue(event.getHandRoot())))
            .addPages(CommandPage.newBuilder()
                .setSequence(destSeq)
                .setCommand(Any.pack(dealCards, "type.googleapis.com/")))
            .build();
    }
    // docs:end:saga_handler
}
