package dev.angzarr.examples.table.sagahand;

import dev.angzarr.*;
import dev.angzarr.client.EventRouter;
import dev.angzarr.examples.*;
import java.util.ArrayList;
import java.util.List;

/**
 * Saga: Table -> Hand (Functional Pattern)
 *
 * <p>Sagas are stateless translators - framework handles sequence stamping. Commands use
 * angzarr_deferred for sequences.
 */
public final class TableHandRouter {

  private TableHandRouter() {}

  // docs:start:event_router
  public static EventRouter createRouter() {
    return new EventRouter("saga-table-hand")
        .domain("table")
        .on(HandStarted.class, TableHandRouter::handleHandStarted);
  }

  // docs:end:event_router

  // docs:start:saga_handler
  public static CommandBook handleHandStarted(HandStarted event, List<EventBook> destinations) {
    // Sagas are stateless - destinations not used, framework stamps sequences
    List<PlayerInHand> players = new ArrayList<>();
    for (SeatSnapshot seat : event.getActivePlayersList()) {
      players.add(
          PlayerInHand.newBuilder()
              .setPlayerRoot(seat.getPlayerRoot())
              .setPosition(seat.getPosition())
              .setStack(seat.getStack())
              .build());
    }

    DealCards dealCards =
        DealCards.newBuilder()
            .setTableRoot(event.getHandRoot())
            .setHandNumber(event.getHandNumber())
            .setGameVariant(event.getGameVariant())
            .setDealerPosition(event.getDealerPosition())
            .setSmallBlind(event.getSmallBlind())
            .setBigBlind(event.getBigBlind())
            .addAllPlayers(players)
            .build();

    return CommandBook.newBuilder()
        .setCover(
            Cover.newBuilder()
                .setDomain("hand")
                .setRoot(UUID.newBuilder().setValue(event.getHandRoot())))
        .addPages(
            CommandPage.newBuilder()
                .setHeader(
                    PageHeader.newBuilder()
                        .setAngzarrDeferred(AngzarrDeferredSequence.newBuilder().build())
                        .build())
                .setCommand(EventRouter.packCommand(dealCards)))
        .build();
  }
  // docs:end:saga_handler
}
