package dev.angzarr.examples.hand.sagatable;

import com.google.protobuf.Any;
import dev.angzarr.*;
import dev.angzarr.client.router.EventRouter;
import dev.angzarr.examples.*;

import java.util.ArrayList;
import java.util.List;

/**
 * Saga: Hand -> Table (Functional Pattern)
 *
 * <p>Reacts to HandComplete events from Hand domain.
 * Sends EndHand commands to Table domain.
 */
public final class HandTableRouter {

    private HandTableRouter() {}

    public static EventRouter createRouter() {
        return new EventRouter("saga-hand-table", "hand")
            .sends("table", "EndHand")
            .prepare(HandComplete.class, HandTableRouter::prepareHandComplete)
            .on(HandComplete.class, HandTableRouter::handleHandComplete);
    }

    public static List<Cover> prepareHandComplete(HandComplete event) {
        return List.of(
            Cover.newBuilder()
                .setDomain("table")
                .setRoot(UUID.newBuilder().setValue(event.getTableRoot()))
                .build()
        );
    }

    public static CommandBook handleHandComplete(HandComplete event, List<EventBook> destinations) {
        int destSeq = EventRouter.nextSequence(destinations.isEmpty() ? null : destinations.get(0));

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
            .setHandRoot(event.getTableRoot())
            .addAllResults(results)
            .build();

        return CommandBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain("table")
                .setRoot(UUID.newBuilder().setValue(event.getTableRoot())))
            .addPages(CommandPage.newBuilder()
                .setSequence(destSeq)
                .setCommand(EventRouter.packCommand(endHand)))
            .build();
    }
}
