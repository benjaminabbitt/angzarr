package dev.angzarr.examples.table;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.InvalidProtocolBufferException;
import dev.angzarr.client.Aggregate;
import dev.angzarr.client.Errors;
import dev.angzarr.client.annotations.Handles;
import dev.angzarr.examples.table.state.SeatState;
import dev.angzarr.examples.table.state.TableState;
import dev.angzarr.examples.*;

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
public class Table extends Aggregate<TableState> {

    public static final String DOMAIN = "table";

    @Override
    public String getDomain() {
        return DOMAIN;
    }

    @Override
    protected TableState createEmptyState() {
        return new TableState();
    }

    @Override
    protected void applyEvent(TableState state, Any eventAny) {
        String typeUrl = eventAny.getTypeUrl();

        try {
            if (typeUrl.endsWith("TableCreated")) {
                TableCreated event = eventAny.unpack(TableCreated.class);
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

            } else if (typeUrl.endsWith("PlayerJoined")) {
                PlayerJoined event = eventAny.unpack(PlayerJoined.class);
                SeatState seat = new SeatState(event.getSeatPosition());
                seat.setPlayerRoot(event.getPlayerRoot().toByteArray());
                seat.setStack(event.getStack());
                seat.setActive(true);
                seat.setSittingOut(false);
                state.getSeats().put(event.getSeatPosition(), seat);

            } else if (typeUrl.endsWith("PlayerLeft")) {
                PlayerLeft event = eventAny.unpack(PlayerLeft.class);
                state.getSeats().remove(event.getSeatPosition());

            } else if (typeUrl.endsWith("PlayerSatOut")) {
                PlayerSatOut event = eventAny.unpack(PlayerSatOut.class);
                SeatState seat = findSeatByPlayer(state, event.getPlayerRoot().toByteArray());
                if (seat != null) {
                    seat.setSittingOut(true);
                }

            } else if (typeUrl.endsWith("PlayerSatIn")) {
                PlayerSatIn event = eventAny.unpack(PlayerSatIn.class);
                SeatState seat = findSeatByPlayer(state, event.getPlayerRoot().toByteArray());
                if (seat != null) {
                    seat.setSittingOut(false);
                }

            } else if (typeUrl.endsWith("HandStarted")) {
                HandStarted event = eventAny.unpack(HandStarted.class);
                state.setStatus("in_hand");
                state.setCurrentHandRoot(event.getHandRoot().toByteArray());
                state.setHandCount(event.getHandNumber());
                state.setDealerPosition(event.getDealerPosition());

            } else if (typeUrl.endsWith("HandEnded")) {
                HandEnded event = eventAny.unpack(HandEnded.class);
                state.setStatus("waiting");
                state.setCurrentHandRoot(new byte[0]);
                // Update stacks from results
                for (var entry : event.getStackChangesMap().entrySet()) {
                    String playerHex = entry.getKey();
                    long delta = entry.getValue();
                    for (SeatState seat : state.getSeats().values()) {
                        if (bytesToHex(seat.getPlayerRoot()).equals(playerHex)) {
                            seat.setStack(seat.getStack() + delta);
                        }
                    }
                }

            } else if (typeUrl.endsWith("ChipsAdded")) {
                ChipsAdded event = eventAny.unpack(ChipsAdded.class);
                SeatState seat = findSeatByPlayer(state, event.getPlayerRoot().toByteArray());
                if (seat != null) {
                    seat.setStack(event.getNewStack());
                }
            }
        } catch (InvalidProtocolBufferException e) {
            throw new RuntimeException("Failed to unpack event: " + typeUrl, e);
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
            throw Errors.CommandRejectedError.preconditionFailed("Player already at table");
        }

        long buyIn = cmd.getBuyInAmount();
        if (buyIn < getState().getMinBuyIn() || buyIn > getState().getMaxBuyIn()) {
            throw Errors.CommandRejectedError.invalidArgument("Buy-in must be between min and max");
        }

        int seatPosition = cmd.getPreferredSeat();
        if (seatPosition < 0 || getState().getSeats().containsKey(seatPosition)) {
            seatPosition = getState().findAvailableSeat();
        }
        if (seatPosition < 0) {
            throw Errors.CommandRejectedError.preconditionFailed("No available seats");
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
            throw Errors.CommandRejectedError.preconditionFailed("Player not at table");
        }
        if (isInHand()) {
            throw Errors.CommandRejectedError.preconditionFailed("Cannot leave during active hand");
        }

        return PlayerLeft.newBuilder()
            .setPlayerRoot(cmd.getPlayerRoot())
            .setSeatPosition(seat.getPosition())
            .setChipsCashedOut(seat.getStack())
            .setLeftAt(now())
            .build();
    }

    @Handles(SitOut.class)
    public PlayerSatOut sitOut(SitOut cmd) {
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
        }
        SeatState seat = getState().findSeatByPlayer(cmd.getPlayerRoot().toByteArray());
        if (seat == null) {
            throw Errors.CommandRejectedError.preconditionFailed("Player not at table");
        }
        if (seat.isSittingOut()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player already sitting out");
        }

        return PlayerSatOut.newBuilder()
            .setPlayerRoot(cmd.getPlayerRoot())
            .setSatOutAt(now())
            .build();
    }

    @Handles(SitIn.class)
    public PlayerSatIn sitIn(SitIn cmd) {
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
        }
        SeatState seat = getState().findSeatByPlayer(cmd.getPlayerRoot().toByteArray());
        if (seat == null) {
            throw Errors.CommandRejectedError.preconditionFailed("Player not at table");
        }
        if (!seat.isSittingOut()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player not sitting out");
        }

        return PlayerSatIn.newBuilder()
            .setPlayerRoot(cmd.getPlayerRoot())
            .setSatInAt(now())
            .build();
    }

    @Handles(StartHand.class)
    public HandStarted startHand(StartHand cmd) {
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
        }
        if (isInHand()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand already in progress");
        }
        if (getActivePlayerCount() < 2) {
            throw Errors.CommandRejectedError.preconditionFailed("Need at least 2 active players");
        }

        long handNumber = getState().getHandCount() + 1;
        int dealerPosition = getState().advanceDealerPosition();

        // Generate hand root
        byte[] handRoot = generateHandRoot(getState().getTableId(), handNumber);

        // Build active player snapshots
        List<SeatSnapshot> activePlayers = new ArrayList<>();
        for (SeatState seat : getState().getSeats().values()) {
            if (seat.isActive()) {
                activePlayers.add(SeatSnapshot.newBuilder()
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

        return HandEnded.newBuilder()
            .setHandRoot(cmd.getHandRoot())
            .addAllResults(cmd.getResultsList())
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
            throw Errors.CommandRejectedError.preconditionFailed("Player not at table");
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

    private static String bytesToHex(byte[] bytes) {
        if (bytes == null) return "";
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
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
            return new int[]{dealerPosition, (dealerPosition + 1) % getState().getMaxPlayers()};
        }
        int sb = (dealerPosition + 1) % getState().getMaxPlayers();
        int bb = (dealerPosition + 2) % getState().getMaxPlayers();
        return new int[]{sb, bb};
    }
}
