package dev.angzarr.examples.table.handlers;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Timestamp;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.table.state.SeatState;
import dev.angzarr.examples.table.state.TableState;
import dev.angzarr.examples.*;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

/**
 * Functional handlers for Table aggregate commands.
 */
public final class TableHandlers {

    private TableHandlers() {}

    // --- State Builder ---

    public static TableState fromEventBook(EventBook eventBook) {
        TableState state = new TableState();
        if (eventBook == null) return state;

        for (EventPage page : eventBook.getPagesList()) {
            applyEvent(state, page.getEvent());
        }
        return state;
    }

    public static void applyEvent(TableState state, Any eventAny) {
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

            } else if (typeUrl.endsWith("PlayerJoined")) {
                PlayerJoined event = eventAny.unpack(PlayerJoined.class);
                SeatState seat = new SeatState(event.getSeatPosition());
                seat.setPlayerRoot(event.getPlayerRoot().toByteArray());
                seat.setStack(event.getStack());
                seat.setActive(true);
                state.getSeats().put(event.getSeatPosition(), seat);

            } else if (typeUrl.endsWith("PlayerLeft")) {
                PlayerLeft event = eventAny.unpack(PlayerLeft.class);
                state.getSeats().remove(event.getSeatPosition());

            } else if (typeUrl.endsWith("PlayerSatOut")) {
                PlayerSatOut event = eventAny.unpack(PlayerSatOut.class);
                SeatState seat = state.findSeatByPlayer(event.getPlayerRoot().toByteArray());
                if (seat != null) seat.setSittingOut(true);

            } else if (typeUrl.endsWith("PlayerSatIn")) {
                PlayerSatIn event = eventAny.unpack(PlayerSatIn.class);
                SeatState seat = state.findSeatByPlayer(event.getPlayerRoot().toByteArray());
                if (seat != null) seat.setSittingOut(false);

            } else if (typeUrl.endsWith("HandStarted")) {
                HandStarted event = eventAny.unpack(HandStarted.class);
                state.setStatus("in_hand");
                state.setCurrentHandRoot(event.getHandRoot().toByteArray());
                state.setHandCount(event.getHandNumber());
                state.setDealerPosition(event.getDealerPosition());

            } else if (typeUrl.endsWith("HandEnded")) {
                state.setStatus("waiting");
                state.setCurrentHandRoot(new byte[0]);

            } else if (typeUrl.endsWith("ChipsAdded")) {
                ChipsAdded event = eventAny.unpack(ChipsAdded.class);
                SeatState seat = state.findSeatByPlayer(event.getPlayerRoot().toByteArray());
                if (seat != null) seat.setStack(event.getNewStack());
            }
        } catch (InvalidProtocolBufferException e) {
            throw new RuntimeException("Failed to unpack event: " + typeUrl, e);
        }
    }

    // --- Command Handlers ---

    public static TableCreated handleCreate(CreateTable cmd, TableState state) {
        if (state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Table already exists");
        }
        if (cmd.getTableName().isEmpty()) {
            throw Errors.CommandRejectedError.invalidArgument("table_name is required");
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

    public static PlayerJoined handleJoin(JoinTable cmd, TableState state) {
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
        }
        if (state.getPlayerCount() >= state.getMaxPlayers()) {
            throw Errors.CommandRejectedError.preconditionFailed("Table is full");
        }

        int seatPosition = cmd.getPreferredSeat() >= 0 ? cmd.getPreferredSeat() : state.findAvailableSeat();

        return PlayerJoined.newBuilder()
            .setPlayerRoot(cmd.getPlayerRoot())
            .setSeatPosition(seatPosition)
            .setBuyInAmount(cmd.getBuyInAmount())
            .setStack(cmd.getBuyInAmount())
            .setJoinedAt(now())
            .build();
    }

    public static PlayerLeft handleLeave(LeaveTable cmd, TableState state) {
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
        }
        SeatState seat = state.findSeatByPlayer(cmd.getPlayerRoot().toByteArray());
        if (seat == null) {
            throw Errors.CommandRejectedError.preconditionFailed("Player not at table");
        }

        return PlayerLeft.newBuilder()
            .setPlayerRoot(cmd.getPlayerRoot())
            .setSeatPosition(seat.getPosition())
            .setChipsCashedOut(seat.getStack())
            .setLeftAt(now())
            .build();
    }

    public static HandStarted handleStartHand(StartHand cmd, TableState state) {
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
        }
        if (state.isInHand()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand already in progress");
        }
        if (state.getActivePlayerCount() < 2) {
            throw Errors.CommandRejectedError.preconditionFailed("Need at least 2 active players");
        }

        long handNumber = state.getHandCount() + 1;
        int dealerPosition = state.advanceDealerPosition();
        byte[] handRoot = generateHandRoot(state.getTableId(), handNumber);

        List<SeatSnapshot> activePlayers = new ArrayList<>();
        for (SeatState seat : state.getSeats().values()) {
            if (seat.isActive()) {
                activePlayers.add(SeatSnapshot.newBuilder()
                    .setPosition(seat.getPosition())
                    .setPlayerRoot(ByteString.copyFrom(seat.getPlayerRoot()))
                    .setStack(seat.getStack())
                    .build());
            }
        }

        return HandStarted.newBuilder()
            .setHandRoot(ByteString.copyFrom(handRoot))
            .setHandNumber(handNumber)
            .setDealerPosition(dealerPosition)
            .setSmallBlindPosition((dealerPosition + 1) % state.getMaxPlayers())
            .setBigBlindPosition((dealerPosition + 2) % state.getMaxPlayers())
            .addAllActivePlayers(activePlayers)
            .setGameVariant(GameVariant.forNumber(state.getGameVariant()))
            .setSmallBlind(state.getSmallBlind())
            .setBigBlind(state.getBigBlind())
            .setStartedAt(now())
            .build();
    }

    public static HandEnded handleEndHand(EndHand cmd, TableState state) {
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Table does not exist");
        }
        if (!state.isInHand()) {
            throw Errors.CommandRejectedError.preconditionFailed("No hand in progress");
        }

        return HandEnded.newBuilder()
            .setHandRoot(cmd.getHandRoot())
            .addAllResults(cmd.getResultsList())
            .setEndedAt(now())
            .build();
    }

    // --- Utilities ---

    private static Timestamp now() {
        Instant instant = Instant.now();
        return Timestamp.newBuilder()
            .setSeconds(instant.getEpochSecond())
            .setNanos(instant.getNano())
            .build();
    }

    private static byte[] generateHandRoot(String tableId, long handNumber) {
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            md.update(tableId.getBytes());
            md.update(String.valueOf(handNumber).getBytes());
            byte[] hash = md.digest();
            byte[] result = new byte[16];
            System.arraycopy(hash, 0, result, 0, 16);
            return result;
        } catch (NoSuchAlgorithmException e) {
            return UUID.randomUUID().toString().getBytes();
        }
    }
}
