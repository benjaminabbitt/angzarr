using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Fluent state reconstruction router.
/// Register handlers once at startup, call WithEvents() per rebuild.
/// </summary>
/// <typeparam name="TState">The state type to build</typeparam>
/// <example>
/// <code>
/// var stateRouter = new StateRouter&lt;PlayerState&gt;()
///     .On&lt;PlayerRegistered&gt;((state, evt) => {
///         state.PlayerId = $"player_{evt.Email}";
///         state.DisplayName = evt.DisplayName;
///     })
///     .On&lt;FundsDeposited&gt;((state, evt) => {
///         if (evt.NewBalance != null)
///             state.Bankroll = evt.NewBalance.Amount;
///     });
///
/// // Use with CommandRouter
/// var router = new CommandRouter("player", stateRouter.ToStateBuilder())
///     .On&lt;RegisterPlayer&gt;(RegisterHandler.Handle);
///
/// // Or use directly
/// var state = stateRouter.WithEventBook(eventBook);
/// </code>
/// </example>
public class StateRouter<TState> where TState : class, new()
{
    private readonly List<(string suffix, Type eventType, Delegate applier)> _handlers = new();
    private readonly Func<TState>? _factory;

    /// <summary>
    /// Create a new StateRouter using default constructor for state.
    /// </summary>
    public StateRouter()
    {
        _factory = null;
    }

    /// <summary>
    /// Create a StateRouter with a custom state factory.
    /// Use when state needs non-default initialization.
    /// </summary>
    public StateRouter(Func<TState> factory)
    {
        _factory = factory;
    }

    /// <summary>
    /// Register an event applier for a specific event type.
    /// </summary>
    public StateRouter<TState> On<TEvent>(Action<TState, TEvent> applier) where TEvent : IMessage, new()
    {
        var suffix = typeof(TEvent).Name;
        _handlers.Add((suffix, typeof(TEvent), applier));
        return this;
    }

    /// <summary>
    /// Create fresh state and apply all events from pages.
    /// </summary>
    public TState WithEvents(IEnumerable<EventPage> pages)
    {
        var state = CreateState();
        foreach (var page in pages)
        {
            if (page.Event != null)
            {
                ApplySingle(state, page.Event);
            }
        }
        return state;
    }

    /// <summary>
    /// Create fresh state from an EventBook.
    /// </summary>
    public TState WithEventBook(EventBook? eventBook)
    {
        if (eventBook == null)
        {
            return CreateState();
        }
        return WithEvents(eventBook.Pages);
    }

    /// <summary>
    /// Apply a single event to existing state.
    /// </summary>
    public void ApplySingle(TState state, Any eventAny)
    {
        var typeUrl = eventAny.TypeUrl;

        foreach (var (suffix, eventType, applier) in _handlers)
        {
            if (typeUrl.EndsWith(suffix))
            {
                var evt = eventAny.Unpack(eventType);
                applier.DynamicInvoke(state, evt);
                return;
            }
        }
        // Unknown event type - silently ignore for forward compatibility
    }

    /// <summary>
    /// Convert to a state builder function for use with CommandRouter.
    /// </summary>
    public Func<EventBook, object> ToStateBuilder()
    {
        return eventBook => WithEventBook(eventBook)!;
    }

    private TState CreateState()
    {
        return _factory != null ? _factory() : new TState();
    }
}
