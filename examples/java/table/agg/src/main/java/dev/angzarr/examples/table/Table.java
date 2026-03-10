package dev.angzarr.examples.table;

import com.google.protobuf.ByteString;
import dev.angzarr.client.CommandHandler;
import dev.angzarr.client.Errors;
import dev.angzarr.client.annotations.Applies;
import dev.angzarr.client.annotations.Handles;
import dev.angzarr.client.util.ByteUtils;
import dev.angzarr.examples.*;
import dev.angzarr.examples.table.state.SeatState;
import dev.angzarr.examples.table.state.TableState;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

/**
 * Table aggregate with event sourcing (OO pattern).
 *
 * <p>Manages game session, seating, and hand lifecycle.
 */
public class Table extends CommandHandler<TableState> {

  public static final String DOMAIN = "table";

  /** Default constructor. */
  public Table() {
    super();
  }

  /** Constructor with prior events for state rehydration. */
  public Table(dev.angzarr.EventBook eventBook) {
    super(eventBook);
  }

  @Override
  public String getDomain() {
    return DOMAIN;
  }

  @Override
  protected TableState createEmptyState() {
    return new TableState();
  }

  // --- Event appliers ---

  @Applies(TableCreated.class)
  public void applyTableCreated(TableState state, TableCreated event) {
    state.setTableId("table_" + event.getTableName());
    state.setTableName(event.getTableName());
    state.setGameVariant(event.getGameVariantValue());
    state.setSmallBlind(event.getSmallBlind());
    state.setBigBlind(event.getBigBlind());
    state.setMinBuyIn(event.getMinBuyIn());
    state.setMaxBuyIn(event.getMaxBuyIn());
    state.setMaxPlayers(event.getMaxPlayers());
    state.setActionTimeoutSeconds(event.getActionTimeoutSeconds());
    state.setStatus("waiting");
    state.setDealerPosition(0);
    state.setHandCount(0);
  }

  @Applies(PlayerJoined.class)
  public void applyPlayerJoined(TableState state, PlayerJoined event) {
    SeatState seat = new SeatState(event.getSeatPosition());
    seat.setPlayerRoot(event.getPlayerRoot().toByteArray());
    seat.setStack(event.getStack());
    seat.setActive(true);
    seat.setSittingOut(false);
    state.getSeats().put(event.getSeatPosition(), seat);
  }

  @Applies(PlayerLeft.class)
  public void applyPlayerLeft(TableState state, PlayerLeft event) {
    state.getSeats().remove(event.getSeatPosition());
  }

  @Applies(PlayerSatOut.class)
  public void applyPlayerSatOut(TableState state, PlayerSatOut event) {
    SeatState seat = findSeatByPlayer(state, event.getPlayerRoot().toByteArray());
    if (seat != null) {
      seat.setSittingOut(true);
    }
  }

  @Applies(PlayerSatIn.class)
  public void applyPlayerSatIn(TableState state, PlayerSatIn event) {
    SeatState seat = findSeatByPlayer(state, event.getPlayerRoot().toByteArray());
    if (seat != null) {
      seat.setSittingOut(false);
    }
  }

  @Applies(HandStarted.class)
  public void applyHandStarted(TableState state, HandStarted event) {
    state.setStatus("in_hand");
    state.setCurrentHandRoot(event.getHandRoot().toByteArray());
    state.setHandCount(event.getHandNumber());
    state.setDealerPosition(event.getDealerPosition());
  }

  @Applies(HandEnded.class)
  public void applyHandEnded(TableState state, HandEnded event) {
    state.setStatus("waiting");
    state.setCurrentHandRoot(new byte[0]);
    // Update stacks from results
    for (var entry : event.getStackChangesMap().entrySet()) {
      String playerHex = entry.getKey();
      long delta = entry.getValue();
      for (SeatState seat : state.getSeats().values()) {
        if (ByteUtils.bytesToHex(seat.getPlayerRoot()).equals(playerHex)) {
          seat.setStack(seat.getStack() + delta);
        }
      }
    }
  }

  @Applies(ChipsAdded.class)
  public void applyChipsAdded(TableState state, ChipsAdded event) {
    SeatState seat = findSeatByPlayer(state, event.getPlayerRoot().toByteArray());
    if (seat != null) {
      seat.setStack(event.getNewStack());
    }
  }

  // --- State accessors ---

  public boolean exists() {
    return getState().exists();
  }

  public String getTableId() {
    return getState().getTableId();
  }

  public String getTableName() {
    return getState().getTableName();
  }

  public int getPlayerCount() {
    return getState().getPlayerCount();
  }

  public int getActivePlayerCount() {
    return getState().getActivePlayerCount();
  }

  public boolean isInHand() {
    return getState().isInHand();
  }

  public long getHandCount() {
    return getState().getHandCount();
  }

  public long getHandNumber() {
    return getState().getHandCount();
  }

  public String getStatus() {
    return getState().getStatus();
  }

  public SeatState getPlayerAtSeat(int position) {
    return getState().getSeats().get(position);
  }

  // --- Command handlers ---

  @Handles(CreateTable.class)
  public TableCreated create(CreateTable cmd) {
    if (exists()) {
      throw Errors.CommandRejectedError.preconditionFailed("Table already exists");
    }
    if (cmd.getTableName().isEmpty()) {
      throw Errors.CommandRejectedError.invalidArgument("table_name is required");
    }
    if (cmd.getMaxPlayers() < 2 || cmd.getMaxPlayers() > 10) {
      throw Errors.CommandRejectedError.invalidArgument("max_players must be between 2 and 10");
    }
    if (cmd.getSmallBlind() <= 0 || cmd.getBigBlind() <= 0) {
      throw Errors.CommandRejectedError.invalidArgument("blinds must be positive");
    }

    return TableCreated.newBuilder()
        .setTableName(cmd.getTableName())
        .setGameVariant(cmd.getGameVariant())
        .setSmallBlind(cmd.getSmallBlind())
        .setBigBlind(cmd.getBigBlind())
        .setMinBuyIn(cmd.getMinBuyIn())
        .setMaxBuyIn(cmd.getMaxBuyIn())
        .setMaxPlayers(cmd.getMaxPlayers())
        .setActionTimeoutSeconds(cmd.getActionTimeoutSeconds())
        .setCreatedAt(now())
        .build();
  }

  @Handles(JoinTable.class)
  public PlayerJoined join(JoinTable cmd) {
    if (!exists()) {
      throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
    }
    if (getPlayerCount() >= getState().getMaxPlayers()) {
      throw Errors.CommandRejectedError.preconditionFailed("Table is full");
    }
    if (getState().findSeatByPlayer(cmd.getPlayerRoot().toByteArray()) != null) {
      throw Errors.CommandRejectedError.preconditionFailed("Player already seated at this table");
    }

    long buyIn = cmd.getBuyInAmount();
    if (buyIn < getState().getMinBuyIn() || buyIn > getState().getMaxBuyIn()) {
      throw Errors.CommandRejectedError.invalidArgument(
          "Buy-in must be at least " + getState().getMinBuyIn());
    }

    int seatPosition = cmd.getPreferredSeat();
    if (seatPosition >= 0) {
      // Specific seat requested - fail if occupied
      if (getState().getSeats().containsKey(seatPosition)) {
        throw Errors.CommandRejectedError.preconditionFailed("Seat is occupied");
      }
    } else {
      // No preference - find any available seat
      seatPosition = getState().findAvailableSeat();
      if (seatPosition < 0) {
        throw Errors.CommandRejectedError.preconditionFailed("No available seats");
      }
    }

    return PlayerJoined.newBuilder()
        .setPlayerRoot(cmd.getPlayerRoot())
        .setSeatPosition(seatPosition)
        .setBuyInAmount(buyIn)
        .setStack(buyIn)
        .setJoinedAt(now())
        .build();
  }

  @Handles(LeaveTable.class)
  public PlayerLeft leave(LeaveTable cmd) {
    if (!exists()) {
      throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
    }
    SeatState seat = getState().findSeatByPlayer(cmd.getPlayerRoot().toByteArray());
    if (seat == null) {
      throw Errors.CommandRejectedError.preconditionFailed("Player is not seated at this table");
    }
    if (isInHand()) {
      throw Errors.CommandRejectedError.preconditionFailed("Cannot leave during a hand");
    }

    return PlayerLeft.newBuilder()
        .setPlayerRoot(cmd.getPlayerRoot())
        .setSeatPosition(seat.getPosition())
        .setChipsCashedOut(seat.getStack())
        .setLeftAt(now())
        .build();
  }

  // Note: SitOut/SitIn commands are in the player domain (player owns intent)
  // Table receives PlayerSatOut/PlayerSatIn as facts via saga

  @Handles(StartHand.class)
  public HandStarted startHand(StartHand cmd) {
    if (!exists()) {
      throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
    }
    if (isInHand()) {
      throw Errors.CommandRejectedError.preconditionFailed("Hand already in progress");
    }
    if (getActivePlayerCount() < 2) {
      throw Errors.CommandRejectedError.preconditionFailed("Not enough players to start a hand");
    }

    long handNumber = getState().getHandCount() + 1;
    int dealerPosition = getState().advanceDealerPosition();

    // Generate hand root
    byte[] handRoot = generateHandRoot(getState().getTableId(), handNumber);

    // Build active player snapshots
    List<SeatSnapshot> activePlayers = new ArrayList<>();
    for (SeatState seat : getState().getSeats().values()) {
      if (seat.isActive()) {
        activePlayers.add(
            SeatSnapshot.newBuilder()
                .setPosition(seat.getPosition())
                .setPlayerRoot(ByteString.copyFrom(seat.getPlayerRoot()))
                .setStack(seat.getStack())
                .build());
      }
    }

    // Calculate blind positions
    int[] positions = calculateBlindPositions(dealerPosition, activePlayers.size());

    return HandStarted.newBuilder()
        .setHandRoot(ByteString.copyFrom(handRoot))
        .setHandNumber(handNumber)
        .setDealerPosition(dealerPosition)
        .setSmallBlindPosition(positions[0])
        .setBigBlindPosition(positions[1])
        .addAllActivePlayers(activePlayers)
        .setGameVariant(GameVariant.forNumber(getState().getGameVariant()))
        .setSmallBlind(getState().getSmallBlind())
        .setBigBlind(getState().getBigBlind())
        .setStartedAt(now())
        .build();
  }

  @Handles(EndHand.class)
  public HandEnded endHand(EndHand cmd) {
    if (!exists()) {
      throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
    }
    if (!isInHand()) {
      throw Errors.CommandRejectedError.preconditionFailed("No hand in progress");
    }

    // Calculate stack changes from results
    java.util.Map<String, Long> stackChanges = new java.util.HashMap<>();
    for (PotResult result : cmd.getResultsList()) {
      String playerHex = ByteUtils.bytesToHex(result.getWinnerRoot().toByteArray());
      long currentChange = stackChanges.getOrDefault(playerHex, 0L);
      stackChanges.put(playerHex, currentChange + result.getAmount());
    }

    return HandEnded.newBuilder()
        .setHandRoot(cmd.getHandRoot())
        .addAllResults(cmd.getResultsList())
        .putAllStackChanges(stackChanges)
        .setEndedAt(now())
        .build();
  }

  @Handles(AddChips.class)
  public ChipsAdded addChips(AddChips cmd) {
    if (!exists()) {
      throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
    }
    SeatState seat = getState().findSeatByPlayer(cmd.getPlayerRoot().toByteArray());
    if (seat == null) {
      throw Errors.CommandRejectedError.preconditionFailed("Player is not seated at this table");
    }
    if (cmd.getAmount() <= 0) {
      throw Errors.CommandRejectedError.invalidArgument("amount must be positive");
    }
    if (isInHand()) {
      throw Errors.CommandRejectedError.preconditionFailed("Cannot add chips during hand");
    }

    long newStack = seat.getStack() + cmd.getAmount();
    if (newStack > getState().getMaxBuyIn()) {
      throw Errors.CommandRejectedError.preconditionFailed("Stack would exceed max buy-in");
    }

    return ChipsAdded.newBuilder()
        .setPlayerRoot(cmd.getPlayerRoot())
        .setAmount(cmd.getAmount())
        .setNewStack(newStack)
        .setAddedAt(now())
        .build();
  }

  // --- Helper methods ---

  private SeatState findSeatByPlayer(TableState state, byte[] playerRoot) {
    return state.findSeatByPlayer(playerRoot);
  }

  private byte[] generateHandRoot(String tableId, long handNumber) {
    try {
      MessageDigest md = MessageDigest.getInstance("SHA-256");
      md.update(tableId.getBytes());
      md.update(String.valueOf(handNumber).getBytes());
      byte[] hash = md.digest();
      byte[] result = new byte[16];
      System.arraycopy(hash, 0, result, 0, 16);
      return result;
    } catch (NoSuchAlgorithmException e) {
      return UUID.randomUUID().toString().replace("-", "").substring(0, 32).getBytes();
    }
  }

  private int[] calculateBlindPositions(int dealerPosition, int playerCount) {
    // Heads-up: dealer is small blind, other is big blind
    // 3+: left of dealer is small blind, left of SB is big blind
    if (playerCount == 2) {
      return new int[] {dealerPosition, (dealerPosition + 1) % getState().getMaxPlayers()};
    }
    int sb = (dealerPosition + 1) % getState().getMaxPlayers();
    int bb = (dealerPosition + 2) % getState().getMaxPlayers();
    return new int[] {sb, bb};
  }
}
