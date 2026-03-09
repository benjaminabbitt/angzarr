package dev.angzarr.examples.table.sagaplayer;

import com.google.protobuf.ByteString;
import dev.angzarr.*;
import dev.angzarr.client.EventRouter;
import dev.angzarr.examples.*;
import java.util.List;

/**
 * Saga: Table -> Player (Functional Pattern)
 *
 * <p>Reacts to HandEnded events from Table domain. Sends ReleaseFunds commands to Player domain.
 * Sagas are stateless translators - framework handles sequence stamping.
 */
public final class TablePlayerRouter {

  private TablePlayerRouter() {}

  public static EventRouter createRouter() {
    return new EventRouter("saga-table-player")
        .domain("table")
        .on(HandEnded.class, TablePlayerRouter::handleHandEnded);
  }

  public static CommandBook handleHandEnded(HandEnded event, List<EventBook> destinations) {
    // Sagas are stateless - destinations not used, framework stamps sequences
    var stackChanges = event.getStackChangesMap();
    if (stackChanges.isEmpty()) {
      return null;
    }

    // Get the first player from the stack changes
    String playerHex = stackChanges.keySet().iterator().next();
    byte[] playerRoot = hexToBytes(playerHex);

    ReleaseFunds releaseFunds = ReleaseFunds.newBuilder().setTableRoot(event.getHandRoot()).build();

    return CommandBook.newBuilder()
        .setCover(
            Cover.newBuilder()
                .setDomain("player")
                .setRoot(UUID.newBuilder().setValue(ByteString.copyFrom(playerRoot))))
        .addPages(
            CommandPage.newBuilder()
                .setHeader(
                    PageHeader.newBuilder()
                        .setAngzarrDeferred(AngzarrDeferredSequence.newBuilder().build())
                        .build())
                .setCommand(EventRouter.packCommand(releaseFunds)))
        .build();
  }

  private static byte[] hexToBytes(String hex) {
    int len = hex.length();
    byte[] data = new byte[len / 2];
    for (int i = 0; i < len; i += 2) {
      data[i / 2] =
          (byte)
              ((Character.digit(hex.charAt(i), 16) << 4) + Character.digit(hex.charAt(i + 1), 16));
    }
    return data;
  }
}
