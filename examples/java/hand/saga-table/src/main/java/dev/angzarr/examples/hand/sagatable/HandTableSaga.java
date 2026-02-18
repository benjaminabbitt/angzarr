package dev.angzarr.examples.hand.sagatable;

import com.google.protobuf.Any;
import dev.angzarr.*;
import dev.angzarr.client.Saga;
import dev.angzarr.client.annotations.Prepares;
import dev.angzarr.client.annotations.ReactsTo;
import dev.angzarr.examples.*;

import java.util.ArrayList;
import java.util.List;

/**
 * Saga: Hand -> Table (OO Pattern)
 *
 * <p>Reacts to HandComplete events from Hand domain.
 * Sends EndHand commands to Table domain.
 */
public class HandTableSaga extends Saga {

    @Override
    public String getName() {
        return "saga-hand-table";
    }

    @Override
    public String getInputDomain() {
        return "hand";
    }

    @Override
    public String getOutputDomain() {
        return "table";
    }

    @Prepares(HandComplete.class)
    public List<Cover> prepareHandComplete(HandComplete event) {
        return List.of(
            Cover.newBuilder()
                .setDomain("table")
                .setRoot(UUID.newBuilder().setValue(event.getTableRoot()))
                .build()
        );
    }

    @ReactsTo(HandComplete.class)
    public CommandBook handleHandComplete(HandComplete event, List<EventBook> destinations) {
        int destSeq = Saga.nextSequence(destinations.isEmpty() ? null : destinations.get(0));

        // Convert PotWinner to PotResult
        List<PotResult> results = new ArrayList<>();
        for (PotWinner winner : event.getWinnersList()) {
            results.add(PotResult.newBuilder()
                .setWinnerRoot(winner.getPlayerRoot())
                .setAmount(winner.getAmount())
                .setPotType(winner.getPotType())
                .setWinningHand(winner.getWinningHand())
                .build());
        }

        EndHand endHand = EndHand.newBuilder()
            .setHandRoot(event.getTableRoot()) // hand_root from the source event
            .addAllResults(results)
            .build();

        return CommandBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain("table")
                .setRoot(UUID.newBuilder().setValue(event.getTableRoot())))
            .addPages(CommandPage.newBuilder()
                .setSequence(destSeq)
                .setCommand(Any.pack(endHand, "type.googleapis.com/")))
            .build();
    }
}
