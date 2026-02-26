/**
 * Unified router pattern for command handlers, sagas, process managers, and projectors.
 *
 * <h2>Overview</h2>
 *
 * <p>Two router categories based on domain cardinality:
 *
 * <ul>
 *   <li><strong>Single-domain routers</strong>: {@link CommandHandlerRouter} and {@link SagaRouter}
 *       take their domain at construction time.</li>
 *   <li><strong>Multi-domain routers</strong>: {@link ProcessManagerRouter} and
 *       {@link ProjectorRouter} use fluent {@code .domain()} registration.</li>
 * </ul>
 *
 * <h2>Handler Interfaces</h2>
 *
 * <p>Each component type has a corresponding handler interface:
 *
 * <ul>
 *   <li>{@link CommandHandlerDomainHandler} - commands -> events, with state</li>
 *   <li>{@link SagaDomainHandler} - events -> commands, stateless</li>
 *   <li>{@link ProcessManagerDomainHandler} - events -> commands + PM events, with shared state</li>
 *   <li>{@link ProjectorDomainHandler} - events -> external output</li>
 * </ul>
 *
 * <h2>Usage Examples</h2>
 *
 * <h3>Command Handler (single domain - domain in constructor)</h3>
 * <pre>{@code
 * CommandHandlerRouter<PlayerState> router = new CommandHandlerRouter<>(
 *     "player", "player", new PlayerHandler());
 * }</pre>
 *
 * <h3>Saga (single domain - domain in constructor)</h3>
 * <pre>{@code
 * SagaRouter router = new SagaRouter(
 *     "saga-order-fulfillment", "order", new OrderHandler());
 * }</pre>
 *
 * <h3>Process Manager (multi-domain - fluent .domain())</h3>
 * <pre>{@code
 * ProcessManagerRouter<HandFlowState> router = ProcessManagerRouter
 *     .<HandFlowState>create("pmg-hand-flow", "hand-flow", stateRouter::withEventBook)
 *     .domain("order", new OrderPmHandler())
 *     .domain("inventory", new InventoryPmHandler());
 * }</pre>
 *
 * <h3>Projector (multi-domain - fluent .domain())</h3>
 * <pre>{@code
 * ProjectorRouter router = ProjectorRouter.create("prj-output")
 *     .domain("player", new PlayerProjectorHandler())
 *     .domain("hand", new HandProjectorHandler());
 * }</pre>
 *
 * <h2>Subscriptions</h2>
 *
 * <p>All routers provide a {@code subscriptions()} method that returns the
 * domain/type pairs needed for event/command routing configuration.
 *
 * <h2>Two-Phase Protocol</h2>
 *
 * <p>Sagas and process managers use a two-phase protocol:
 * <ol>
 *   <li>{@code prepareDestinations()} - declare what destination state is needed</li>
 *   <li>{@code dispatch()} - execute with fetched destination state</li>
 * </ol>
 *
 * @see CommandHandlerRouter
 * @see SagaRouter
 * @see ProcessManagerRouter
 * @see ProjectorRouter
 */
package dev.angzarr.client.router;
