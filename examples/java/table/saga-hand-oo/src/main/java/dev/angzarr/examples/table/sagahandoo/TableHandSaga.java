// DOC: This file is referenced in docs/docs/examples/sagas.mdx
//      Update documentation when making changes to saga patterns.
package dev.angzarr.examples.table.sagahandoo;

import com.google.protobuf.Any;
import dev.angzarr.*;
import dev.angzarr.client.Saga;
import dev.angzarr.client.annotations.Handles;
import dev.angzarr.client.annotations.Prepares;
import dev.angzarr.examples.*;
import java.util.ArrayList;
import java.util.List;

// docs:start:saga_oo
/**
 * Saga: Table -> Hand (OO Pattern)
 *
 * <p>Reacts to HandStarted events from Table domain. Sends DealCards commands to Hand domain.
 *
 * <p>Uses annotation-based handler registration with:
 *
 * <ul>
 *   <li>{@code @Prepares(EventType.class)} for prepare phase handlers
 *   <li>{@code @Handles(EventType.class)} for execute phase handlers
 * </ul>
 */
public class TableHandSaga extends Saga {

  public TableHandSaga() {
    super("saga-table-hand", "table", "hand");
  }

  /**
   * Prepare phase: declare which destination aggregates we need to read.
   *
   * <p>Called during the prepare phase of the two-phase saga protocol. Returns a list of Cover
   * objects identifying the destination aggregates needed for the execute phase.
   */
  @Prepares(HandStarted.class)
  public List<Cover> prepareHandStarted(HandStarted event) {
    return List.of(
        Cover.newBuilder()
            .setDomain("hand")
            .setRoot(UUID.newBuilder().setValue(event.getHandRoot()))
            .build());
  }

  /**
   * Execute phase: translate Table.HandStarted -> Hand.DealCards.
   *
   * <p>Called during the execute phase with the source event and fetched destination EventBooks.
   * Returns the command to send.
   */
  @Handles(HandStarted.class)
  public CommandBook handleHandStarted(HandStarted event, List<EventBook> destinations) {
    int destSeq = Saga.nextSequence(destinations.isEmpty() ? null : destinations.get(0));

    // Convert SeatSnapshot to PlayerInHand
    List<PlayerInHand> players = new ArrayList<>();
    for (SeatSnapshot seat : event.getActivePlayersList()) {
      players.add(
          PlayerInHand.newBuilder()
              .setPlayerRoot(seat.getPlayerRoot())
              .setPosition(seat.getPosition())
              .setStack(seat.getStack())
              .build());
    }

    // Build DealCards command
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
                .setHeader(PageHeader.newBuilder().setSequence(destSeq).build())
                .setCommand(Any.pack(dealCards, "type.googleapis.com/")))
        .build();
  }
}
// docs:end:saga_oo
