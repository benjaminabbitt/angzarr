package dev.angzarr.examples.player;

import dev.angzarr.EventBook;
import dev.angzarr.client.CommandHandler;
import dev.angzarr.client.annotations.Applies;
import dev.angzarr.client.annotations.Handles;
import dev.angzarr.examples.*;
import dev.angzarr.examples.player.handlers.*;
import dev.angzarr.examples.player.state.PlayerState;

/**
 * Player aggregate with event sourcing (OO pattern).
 *
 * <p>Manages player registration, funds, and table reservations.
 *
 * <p>This OO-style aggregate wraps the functional handlers for use with the annotation-based
 * CommandHandler framework. Both patterns produce identical behavior - choose based on team
 * preference.
 */
public class Player extends CommandHandler<PlayerState> {

  public static final String DOMAIN = "player";

  public Player() {
    super();
  }

  public Player(EventBook eventBook) {
    super(eventBook);
  }

  @Override
  public String getDomain() {
    return DOMAIN;
  }

  @Override
  protected PlayerState createEmptyState() {
    return new PlayerState();
  }

  // --- Event appliers ---

  @Applies(PlayerRegistered.class)
  public void applyPlayerRegistered(PlayerState state, PlayerRegistered event) {
    state.setPlayerId("player_" + event.getEmail());
    state.setDisplayName(event.getDisplayName());
    state.setEmail(event.getEmail());
    state.setPlayerType(event.getPlayerTypeValue());
    state.setAiModelId(event.getAiModelId());
    state.setStatus("active");
    state.setBankroll(0);
    state.setReservedFunds(0);
  }

  @Applies(FundsDeposited.class)
  public void applyFundsDeposited(PlayerState state, FundsDeposited event) {
    if (event.hasNewBalance()) {
      state.setBankroll(event.getNewBalance().getAmount());
    }
  }

  @Applies(FundsWithdrawn.class)
  public void applyFundsWithdrawn(PlayerState state, FundsWithdrawn event) {
    if (event.hasNewBalance()) {
      state.setBankroll(event.getNewBalance().getAmount());
    }
  }

  @Applies(FundsReserved.class)
  public void applyFundsReserved(PlayerState state, FundsReserved event) {
    if (event.hasNewReservedBalance()) {
      state.setReservedFunds(event.getNewReservedBalance().getAmount());
    }
    String tableKey = bytesToHex(event.getTableRoot().toByteArray());
    if (event.hasAmount()) {
      state.getTableReservations().put(tableKey, event.getAmount().getAmount());
    }
  }

  @Applies(FundsReleased.class)
  public void applyFundsReleased(PlayerState state, FundsReleased event) {
    if (event.hasNewReservedBalance()) {
      state.setReservedFunds(event.getNewReservedBalance().getAmount());
    }
    String tableKey = bytesToHex(event.getTableRoot().toByteArray());
    state.getTableReservations().remove(tableKey);
  }

  // --- State accessors ---

  public boolean exists() {
    return getState().exists();
  }

  public String getPlayerId() {
    return getState().getPlayerId();
  }

  public String getDisplayName() {
    return getState().getDisplayName();
  }

  public long getBankroll() {
    return getState().getBankroll();
  }

  public long getReservedFunds() {
    return getState().getReservedFunds();
  }

  public long getAvailableBalance() {
    return getState().getAvailableBalance();
  }

  public String getStatus() {
    return getState().getStatus();
  }

  // --- Command handlers ---

  @Handles(RegisterPlayer.class)
  public PlayerRegistered register(RegisterPlayer cmd) {
    return RegisterHandler.handle(cmd, getState());
  }

  @Handles(DepositFunds.class)
  public FundsDeposited deposit(DepositFunds cmd) {
    return DepositHandler.handle(cmd, getState());
  }

  @Handles(WithdrawFunds.class)
  public FundsWithdrawn withdraw(WithdrawFunds cmd) {
    return WithdrawHandler.handle(cmd, getState());
  }

  @Handles(ReserveFunds.class)
  public FundsReserved reserve(ReserveFunds cmd) {
    return ReserveHandler.handle(cmd, getState());
  }

  @Handles(ReleaseFunds.class)
  public FundsReleased release(ReleaseFunds cmd) {
    return ReleaseHandler.handle(cmd, getState());
  }

  // --- Helper methods ---

  private static String bytesToHex(byte[] bytes) {
    StringBuilder sb = new StringBuilder();
    for (byte b : bytes) {
      sb.append(String.format("%02x", b));
    }
    return sb.toString();
  }
}
