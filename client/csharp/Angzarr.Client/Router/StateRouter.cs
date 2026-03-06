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
///     .On&lt;PlayerRegistered&gt;(PlayerState.ApplyRegistered)
///     .On&lt;FundsDeposited&gt;(PlayerState.ApplyDeposited);
///
/// // Use per rebuild
/// var state = playerRouter.WithEventBook(eventBook);
/// </code>
/// </summary>
public class StateRouter<TState>
    where TState : new()
{
    private readonly Dictionary<string, IEventApplier<TState>> _appliers = new();
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
    /// var router = new StateRouter&lt;HandState&gt;(HandState.CreateInitial);
    /// </code>
    /// </summary>
    /// <param name="factory">Factory function to create initial state.</param>
    public StateRouter(Func<TState> factory)
    {
        _factory = factory;
    }

    /// <summary>
    /// Create a StateRouter with a custom state factory.
    /// Alternative static factory method.
    ///
    /// Example:
    /// <code>
    /// var router = StateRouter&lt;HandState&gt;.WithFactory(HandState.CreateInitial);
    /// </code>
    /// </summary>
    /// <param name="factory">Factory function to create initial state.</param>
    /// <returns>A new StateRouter with the custom factory.</returns>
    public static StateRouter<TState> WithFactory(Func<TState> factory)
    {
        return new StateRouter<TState>(factory);
    }

    private TState CreateState()
    {
        if (_factory != null)
            return _factory();
        return new TState();
    }

    /// <summary>
    /// Register an event applier using a method reference.
    /// </summary>
    public StateRouter<TState> On<TEvent>(Action<TState, TEvent> applier)
        where TEvent : IMessage, new()
    {
        var suffix = typeof(TEvent).Name;
        _appliers[suffix] = new EventApplier<TEvent>(applier);
        return this;
    }

    private interface IEventApplier<in T>
    {
        void Apply(T state, Any eventAny);
    }

    private sealed class EventApplier<TEvent> : IEventApplier<TState>
        where TEvent : IMessage, new()
    {
        private readonly Action<TState, TEvent> _applier;

        public EventApplier(Action<TState, TEvent> applier)
        {
            _applier = applier;
        }

        public void Apply(TState state, Any eventAny)
        {
            var evt = eventAny.Unpack<TEvent>();
            _applier(state, evt);
        }
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

    /// <summary>
    /// Apply a single event to an existing state.
    /// Useful when state is managed externally (e.g., in OO aggregates).
    /// </summary>
    public void ApplySingle(TState state, Any eventAny)
    {
        ApplyEvent(state, eventAny);
    }

    private void ApplyEvent(TState state, Any eventAny)
    {
        // Extract the simple type name from the TypeUrl
        // e.g., "type.googleapis.com/examples.CommunityCardsDealt" -> "CommunityCardsDealt"
        var typeUrl = eventAny.TypeUrl;
        var lastDot = typeUrl.LastIndexOf('.');
        var typeName = lastDot >= 0 ? typeUrl.Substring(lastDot + 1) : typeUrl;

        // Now match exactly by type name
        if (_appliers.TryGetValue(typeName, out var applier))
        {
            applier.Apply(state, eventAny);
            return;
        }
        // Unknown event type - silently ignore (forward compatibility)
    }
}
