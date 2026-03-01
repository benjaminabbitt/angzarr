package dev.angzarr.examples.player.handlers;

import com.google.protobuf.ByteString;
import com.google.protobuf.Timestamp;
import dev.angzarr.Notification;
import dev.angzarr.client.compensation.CompensationContext;
import dev.angzarr.examples.Currency;
import dev.angzarr.examples.FundsReleased;
import dev.angzarr.examples.player.state.PlayerState;
import java.time.Instant;
import java.util.logging.Logger;

/**
 * Functional handler for rejection compensation.
 *
 * <p>Handles JoinTable rejection by releasing reserved funds.
 */
public final class RejectedHandler {

  private static final Logger logger = Logger.getLogger(RejectedHandler.class.getName());

  private RejectedHandler() {}

  /**
   * Handle JoinTable rejection by releasing reserved funds.
   *
   * @param notification The rejection notification
   * @param state Current aggregate state
   * @return The compensation event
   */
  public static FundsReleased handleJoinRejected(Notification notification, PlayerState state) {
    var ctx = CompensationContext.from(notification);

    logger.warning(
        "Player compensation for JoinTable rejection: reason=" + ctx.getRejectionReason());

    // Extract table_root from the rejected command
    byte[] tableRoot = new byte[0];
    if (ctx.getRejectedCommand() != null
        && ctx.getRejectedCommand().getCover() != null
        && ctx.getRejectedCommand().getCover().hasRoot()) {
      tableRoot = ctx.getRejectedCommand().getCover().getRoot().getValue().toByteArray();
    }

    // Release the funds that were reserved for this table
    String tableKey = bytesToHex(tableRoot);
    long reservedAmount = state.getReservationForTable(tableKey);
    long newReserved = state.getReservedFunds() - reservedAmount;
    long newAvailable = state.getBankroll() - newReserved;

    return FundsReleased.newBuilder()
        .setAmount(Currency.newBuilder().setAmount(reservedAmount).setCurrencyCode("CHIPS"))
        .setTableRoot(ByteString.copyFrom(tableRoot))
        .setNewAvailableBalance(
            Currency.newBuilder().setAmount(newAvailable).setCurrencyCode("CHIPS"))
        .setNewReservedBalance(
            Currency.newBuilder().setAmount(newReserved).setCurrencyCode("CHIPS"))
        .setReleasedAt(now())
        .build();
  }

  private static String bytesToHex(byte[] bytes) {
    StringBuilder sb = new StringBuilder();
    for (byte b : bytes) {
      sb.append(String.format("%02x", b));
    }
    return sb.toString();
  }

  private static Timestamp now() {
    Instant instant = Instant.now();
    return Timestamp.newBuilder()
        .setSeconds(instant.getEpochSecond())
        .setNanos(instant.getNano())
        .build();
  }
}
