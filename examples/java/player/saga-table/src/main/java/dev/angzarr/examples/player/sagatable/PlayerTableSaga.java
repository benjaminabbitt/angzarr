package dev.angzarr.examples.player.sagatable;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import dev.angzarr.*;
import dev.angzarr.client.Saga;
import dev.angzarr.client.annotations.Handles;
import dev.angzarr.client.annotations.Prepares;
import dev.angzarr.client.router.SagaHandlerResponse;
import dev.angzarr.examples.*;
import java.util.Collections;
import java.util.List;

/**
 * Saga: Player -> Table
 *
 * <p>Propagates player sit-out/sit-in intent as facts to the table domain.
 *
 * <p>Flow:
 *
 * <ul>
 *   <li>PlayerSittingOut -> PlayerSatOut fact to table
 *   <li>PlayerReturningToPlay -> PlayerSatIn fact to table
 * </ul>
 *
 * <p>Uses annotation-based handler registration.
 */
public class PlayerTableSaga extends Saga {

  // Store source root during dispatch for handler access
  private ByteString currentSourceRoot = ByteString.EMPTY;

  public PlayerTableSaga() {
    super("saga-player-table", "player", "table");
  }

  @Override
  public SagaHandlerResponse dispatch(EventBook book, List<EventBook> destinations) {
    // Store source root for handler access
    if (book.hasCover() && book.getCover().hasRoot()) {
      currentSourceRoot = book.getCover().getRoot().getValue();
    } else {
      currentSourceRoot = ByteString.EMPTY;
    }
    return super.dispatch(book, destinations);
  }

  /** Prepare phase: no destinations needed (emits facts, not commands). */
  @Prepares(PlayerSittingOut.class)
  public List<Cover> prepareSittingOut(PlayerSittingOut event) {
    return Collections.emptyList();
  }

  /** Prepare phase: no destinations needed (emits facts, not commands). */
  @Prepares(PlayerReturningToPlay.class)
  public List<Cover> prepareReturningToPlay(PlayerReturningToPlay event) {
    return Collections.emptyList();
  }

  /** Execute phase: translate PlayerSittingOut -> PlayerSatOut fact for table. */
  @Handles(PlayerSittingOut.class)
  public void handleSittingOut(PlayerSittingOut event) {
    // Create PlayerSatOut fact for the table
    PlayerSatOut satOut =
        PlayerSatOut.newBuilder()
            .setPlayerRoot(currentSourceRoot)
            .setSatOutAt(event.getSatOutAt())
            .build();

    EventBook fact =
        EventBook.newBuilder()
            .setCover(
                Cover.newBuilder()
                    .setDomain("table")
                    .setRoot(UUID.newBuilder().setValue(event.getTableRoot())))
            .addPages(EventPage.newBuilder().setEvent(Any.pack(satOut, "type.googleapis.com/")))
            .build();

    emitFact(fact);
  }

  /** Execute phase: translate PlayerReturningToPlay -> PlayerSatIn fact for table. */
  @Handles(PlayerReturningToPlay.class)
  public void handleReturningToPlay(PlayerReturningToPlay event) {
    // Create PlayerSatIn fact for the table
    PlayerSatIn satIn =
        PlayerSatIn.newBuilder()
            .setPlayerRoot(currentSourceRoot)
            .setSatInAt(event.getSatInAt())
            .build();

    EventBook fact =
        EventBook.newBuilder()
            .setCover(
                Cover.newBuilder()
                    .setDomain("table")
                    .setRoot(UUID.newBuilder().setValue(event.getTableRoot())))
            .addPages(EventPage.newBuilder().setEvent(Any.pack(satIn, "type.googleapis.com/")))
            .build();

    emitFact(fact);
  }
}
