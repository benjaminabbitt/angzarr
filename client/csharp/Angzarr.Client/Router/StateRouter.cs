using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Fluent state reconstruction from events.
///
/// Provides a builder pattern for registering event appliers.
/// Register once at startup, call WithEventBook() per rebuild.
///
/// Example:
/// <code>
/// var playerRouter = new StateRouter&lt;PlayerState&gt;()
///     .On&lt;PlayerRegistered&gt;((state, evt) =&gt; state.PlayerId = evt.PlayerId)
///     .On&lt;FundsDeposited&gt;((state, evt) =&gt; state.Bankroll += evt.Amount);
///
/// // Use per rebuild
/// var state = playerRouter.WithEventBook(eventBook);
/// </code>
/// </summary>
public class StateRouter<TState>
    where TState : new()
{
    private readonly Dictionary<string, (Type EventType, Action<TState, Any> Applier)> _appliers =
        new();
    private readonly Func<TState>? _factory;

    /// <summary>
    /// Create a new StateRouter using default constructor for state creation.
    /// </summary>
    public StateRouter()
    {
        _factory = null;
    }

    /// <summary>
    /// Create a StateRouter with a custom state factory.
    /// Use this when your state needs non-default initialization.
    ///
    /// Example:
    /// <code>
    /// var router = StateRouter&lt;HandState&gt;.WithFactory(() =&gt; new HandState
    /// {
    ///     Pots = new List&lt;PotState&gt; { new PotState { PotType = "main" } }
    /// });
    /// </code>
    /// </summary>
    /// <param name="factory">Factory function to create initial state.</param>
    /// <returns>A new StateRouter with the custom factory.</returns>
    public static StateRouter<TState> WithFactory(Func<TState> factory)
    {
        return new StateRouter<TState>(factory);
    }

    private StateRouter(Func<TState> factory)
    {
        _factory = factory;
    }

    private TState CreateState()
    {
        return _factory != null ? _factory() : new TState();
    }

    /// <summary>
    /// Register an event applier.
    /// </summary>
    public StateRouter<TState> On<TEvent>(Action<TState, TEvent> applier)
        where TEvent : IMessage, new()
    {
        var suffix = typeof(TEvent).Name;
        _appliers[suffix] = (
            typeof(TEvent),
            (state, any) =>
            {
                var evt = any.Unpack<TEvent>();
                applier(state, evt);
            }
        );
        return this;
    }

    /// <summary>
    /// Rebuild state from an EventBook.
    /// </summary>
    public TState WithEventBook(Angzarr.EventBook? book)
    {
        var state = CreateState();
        if (book == null)
            return state;

        foreach (var page in book.Pages)
        {
            if (page.Event == null)
                continue;
            ApplyEvent(state, page.Event);
        }
        return state;
    }

    private void ApplyEvent(TState state, Any eventAny)
    {
        foreach (var (suffix, (_, applier)) in _appliers)
        {
            if (eventAny.TypeUrl.EndsWith(suffix))
            {
                applier(state, eventAny);
                return;
            }
        }
        // Unknown event type - silently ignore (forward compatibility)
    }
}
