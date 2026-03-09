package dev.angzarr.examples.hand.sagaplayer;

import dev.angzarr.*;
import dev.angzarr.client.EventRouter;
import dev.angzarr.examples.*;
import java.util.List;

/**
 * Saga: Hand -> Player (Functional Pattern)
 *
 * <p>Reacts to PotAwarded events from Hand domain. Sends DepositFunds commands to Player domain.
 * Sagas are stateless translators - framework handles sequence stamping.
 */
public final class HandPlayerRouter {

  private HandPlayerRouter() {}

  public static EventRouter createRouter() {
    return new EventRouter("saga-hand-player")
        .domain("hand")
        .on(PotAwarded.class, HandPlayerRouter::handlePotAwarded);
  }

  public static CommandBook handlePotAwarded(PotAwarded event, List<EventBook> destinations) {
    // Sagas are stateless - destinations not used, framework stamps sequences
    if (event.getWinnersCount() == 0) {
      return null;
    }

    // For a single winner, send a single CommandBook
    PotWinner winner = event.getWinners(0);

    DepositFunds depositFunds =
        DepositFunds.newBuilder()
            .setAmount(Currency.newBuilder().setAmount(winner.getAmount()))
            .build();

    return CommandBook.newBuilder()
        .setCover(
            Cover.newBuilder()
                .setDomain("player")
                .setRoot(UUID.newBuilder().setValue(winner.getPlayerRoot())))
        .addPages(
            CommandPage.newBuilder()
                .setHeader(
                    PageHeader.newBuilder()
                        .setAngzarrDeferred(AngzarrDeferredSequence.newBuilder().build())
                        .build())
                .setCommand(EventRouter.packCommand(depositFunds)))
        .build();
  }
}
