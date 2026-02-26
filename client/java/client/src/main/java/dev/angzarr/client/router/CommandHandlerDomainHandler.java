package dev.angzarr.client.router;

import com.google.protobuf.Any;
import dev.angzarr.CommandBook;
import dev.angzarr.EventBook;
import dev.angzarr.Notification;
import dev.angzarr.client.StateRouter;
import dev.angzarr.client.compensation.RejectionHandlerResponse;

import java.util.List;

/**
 * Handler interface for a single domain's command handler.
 *
 * <p>Command handlers receive commands and emit events. They maintain state
 * that is rebuilt from events using a {@link StateRouter}.
 *
 * <p>Example:
 * <pre>{@code
 * public class PlayerHandler implements CommandHandlerDomainHandler<PlayerState> {
 *     private final StateRouter<PlayerState> stateRouter;
 *
 *     public PlayerHandler() {
 *         this.stateRouter = new StateRouter<>(PlayerState::new)
 *             .on(PlayerRegistered.class, (state, event) -> {
 *                 state.setPlayerId(event.getPlayerId());
 *                 state.setDisplayName(event.getDisplayName());
 *             })
 *             .on(FundsDeposited.class, (state, event) -> {
 *                 state.setBankroll(state.getBankroll() + event.getAmount());
 *             });
 *     }
 *
 *     @Override
 *     public List<String> commandTypes() {
 *         return List.of("RegisterPlayer", "DepositFunds");
 *     }
 *
 *     @Override
 *     public StateRouter<PlayerState> stateRouter() {
 *         return stateRouter;
 *     }
 *
 *     @Override
 *     public EventBook handle(CommandBook cmd, Any payload, PlayerState state, int seq)
 *             throws CommandRejectedError {
 *         String typeUrl = payload.getTypeUrl();
 *         if (typeUrl.endsWith("RegisterPlayer")) {
 *             return handleRegister(cmd, payload, state, seq);
 *         } else if (typeUrl.endsWith("DepositFunds")) {
 *             return handleDeposit(cmd, payload, state, seq);
 *         }
 *         throw CommandRejectedError.of("Unknown command: " + typeUrl);
 *     }
 * }
 * }</pre>
 *
 * @param <S> The state type for this command handler
 */
public interface CommandHandlerDomainHandler<S> {

    /**
     * Command type suffixes this handler processes.
     *
     * <p>Used for subscription derivation and routing.
     *
     * @return List of command type suffixes (e.g., "RegisterPlayer", "DepositFunds")
     */
    List<String> commandTypes();

    /**
     * Get the state router for rebuilding state from events.
     */
    StateRouter<S> stateRouter();

    /**
     * Rebuild state from events.
     *
     * <p>Default implementation uses {@code stateRouter().withEventBook()}.
     *
     * @param events The event book containing prior events
     * @return The rebuilt state
     */
    default S rebuild(EventBook events) {
        return stateRouter().withEventBook(events);
    }

    /**
     * Handle a command and return resulting events.
     *
     * <p>The handler should dispatch internally based on {@code payload.getTypeUrl()}.
     *
     * @param cmd The command book containing metadata
     * @param payload The command payload as an Any
     * @param state The current state
     * @param seq The next sequence number for events
     * @return The event book containing resulting events
     * @throws CommandRejectedError if the command is rejected
     */
    EventBook handle(CommandBook cmd, Any payload, S state, int seq) throws CommandRejectedError;

    /**
     * Handle a rejection notification.
     *
     * <p>Called when a command issued by a saga/PM targeting this domain
     * was rejected. Override to provide custom compensation logic.
     *
     * <p>Default implementation returns an empty response (framework handles).
     *
     * @param notification The rejection notification
     * @param state The current state
     * @param targetDomain The domain the rejected command targeted
     * @param targetCommand The rejected command type suffix
     * @return The rejection handler response
     * @throws CommandRejectedError if the rejection cannot be handled
     */
    default RejectionHandlerResponse onRejected(
            Notification notification,
            S state,
            String targetDomain,
            String targetCommand) throws CommandRejectedError {
        return RejectionHandlerResponse.empty();
    }
}
