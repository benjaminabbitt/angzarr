package dev.angzarr.examples.hand.sagatable;

import dev.angzarr.*;
import dev.angzarr.client.EventRouter;
import dev.angzarr.examples.*;
import java.util.ArrayList;
import java.util.List;

/**
 * Saga: Hand -> Table (Functional Pattern)
 *
 * <p>Reacts to HandComplete events from Hand domain. Sends EndHand commands to Table domain. Sagas
 * are stateless translators - framework handles sequence stamping.
 */
public final class HandTableRouter {

  private HandTableRouter() {}

  public static EventRouter createRouter() {
    return new EventRouter("saga-hand-table")
        .domain("hand")
        .on(HandComplete.class, HandTableRouter::handleHandComplete);
  }

  public static CommandBook handleHandComplete(HandComplete event, List<EventBook> destinations) {
    // Sagas are stateless - destinations not used, framework stamps sequences

    // Convert PotWinner to PotResult
    List<PotResult> results = new ArrayList<>();
    for (PotWinner winner : event.getWinnersList()) {
      results.add(
          PotResult.newBuilder()
              .setWinnerRoot(winner.getPlayerRoot())
              .setAmount(winner.getAmount())
              .setPotType(winner.getPotType())
              .setWinningHand(winner.getWinningHand())
              .build());
    }

    EndHand endHand =
        EndHand.newBuilder().setHandRoot(event.getTableRoot()).addAllResults(results).build();

    return CommandBook.newBuilder()
        .setCover(
            Cover.newBuilder()
                .setDomain("table")
                .setRoot(UUID.newBuilder().setValue(event.getTableRoot())))
        .addPages(
            CommandPage.newBuilder()
                .setHeader(
                    PageHeader.newBuilder()
                        .setAngzarrDeferred(AngzarrDeferredSequence.newBuilder().build())
                        .build())
                .setCommand(EventRouter.packCommand(endHand)))
        .build();
  }
}
