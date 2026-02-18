// DOC: This file is referenced in docs/docs/examples/aggregates.mdx
//      Update documentation when making changes to StateBuilder patterns.
package dev.angzarr.examples.player.handlers;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.examples.player.state.PlayerState;
import dev.angzarr.examples.FundsDeposited;
import dev.angzarr.examples.FundsReleased;
import dev.angzarr.examples.FundsReserved;
import dev.angzarr.examples.FundsWithdrawn;
import dev.angzarr.examples.PlayerRegistered;

/**
 * Builds PlayerState from an EventBook (functional style).
 *
 * <p>Used by CommandRouter for state reconstruction.
 */
// docs:start:state_builder
public final class StateBuilder {

    private StateBuilder() {}

    /**
     * Build state from event book by replaying all events.
     */
    public static PlayerState fromEventBook(EventBook eventBook) {
        PlayerState state = new PlayerState();
        if (eventBook == null) {
            return state;
        }

        for (EventPage page : eventBook.getPagesList()) {
            applyEvent(state, page.getEvent());
        }
        return state;
    }

    /**
     * Apply a single event to state.
     */
    public static void applyEvent(PlayerState state, Any eventAny) {
        String typeUrl = eventAny.getTypeUrl();

        try {
            if (typeUrl.endsWith("PlayerRegistered")) {
                PlayerRegistered event = eventAny.unpack(PlayerRegistered.class);
                state.setPlayerId("player_" + event.getEmail());
                state.setDisplayName(event.getDisplayName());
                state.setEmail(event.getEmail());
                state.setPlayerType(event.getPlayerTypeValue());
                state.setAiModelId(event.getAiModelId());
                state.setStatus("active");
                state.setBankroll(0);
                state.setReservedFunds(0);

            } else if (typeUrl.endsWith("FundsDeposited")) {
                FundsDeposited event = eventAny.unpack(FundsDeposited.class);
                if (event.hasNewBalance()) {
                    state.setBankroll(event.getNewBalance().getAmount());
                }

            } else if (typeUrl.endsWith("FundsWithdrawn")) {
                FundsWithdrawn event = eventAny.unpack(FundsWithdrawn.class);
                if (event.hasNewBalance()) {
                    state.setBankroll(event.getNewBalance().getAmount());
                }

            } else if (typeUrl.endsWith("FundsReserved")) {
                FundsReserved event = eventAny.unpack(FundsReserved.class);
                if (event.hasNewReservedBalance()) {
                    state.setReservedFunds(event.getNewReservedBalance().getAmount());
                }
                String tableKey = bytesToHex(event.getTableRoot().toByteArray());
                if (event.hasAmount()) {
                    state.getTableReservations().put(tableKey, event.getAmount().getAmount());
                }

            } else if (typeUrl.endsWith("FundsReleased")) {
                FundsReleased event = eventAny.unpack(FundsReleased.class);
                if (event.hasNewReservedBalance()) {
                    state.setReservedFunds(event.getNewReservedBalance().getAmount());
                }
                String tableKey = bytesToHex(event.getTableRoot().toByteArray());
                state.getTableReservations().remove(tableKey);
            }
        } catch (InvalidProtocolBufferException e) {
            throw new RuntimeException("Failed to unpack event: " + typeUrl, e);
        }
    }

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
// docs:end:state_builder
