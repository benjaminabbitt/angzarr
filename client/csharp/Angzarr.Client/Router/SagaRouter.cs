using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Angzarr.Client.Router;

/// <summary>
/// Router for saga components (events -> commands, single domain, stateless).
///
/// Domain is set at construction time. No Domain() method exists,
/// enforcing single-domain constraint at compile time.
///
/// Example:
/// <code>
/// var router = new SagaRouter&lt;OrderSagaHandler&gt;(
///     "saga-order-fulfillment",
///     "order",
///     new OrderSagaHandler()
/// );
///
/// // Phase 1: Get destinations needed
/// var destinations = router.PrepareDestinations(sourceEventBook);
///
/// // Phase 2: Execute with fetched destinations
/// var response = router.Dispatch(sourceEventBook, fetchedDestinations);
/// </code>
/// </summary>
/// <typeparam name="THandler">The handler implementation type.</typeparam>
public class SagaRouter<THandler>
    where THandler : ISagaDomainHandler
{
    private readonly string _name;
    private readonly string _domain;
    private readonly THandler _handler;

    /// <summary>
    /// Create a new saga router.
    /// Sagas translate events from one domain to commands for another.
    /// Single domain enforced at construction.
    /// </summary>
    /// <param name="name">Component name.</param>
    /// <param name="domain">Input domain this saga listens to.</param>
    /// <param name="handler">Handler implementation.</param>
    public SagaRouter(string name, string domain, THandler handler)
    {
        _name = name;
        _domain = domain;
        _handler = handler;
    }

    /// <summary>
    /// Get the router name.
    /// </summary>
    public string Name => _name;

    /// <summary>
    /// Get the input domain.
    /// </summary>
    public string InputDomain => _domain;

    /// <summary>
    /// Get event types from the handler.
    /// </summary>
    public IReadOnlyList<string> EventTypes() => _handler.EventTypes();

    /// <summary>
    /// Get subscriptions for this saga.
    /// Returns list of (domain, event types) tuples.
    /// </summary>
    public IReadOnlyList<(string Domain, IReadOnlyList<string> Types)> Subscriptions()
    {
        return new[] { (_domain, _handler.EventTypes()) };
    }

    /// <summary>
    /// Get destinations needed for the given source events.
    /// </summary>
    /// <param name="source">Source event book, may be null.</param>
    /// <returns>List of covers identifying needed destination aggregates.</returns>
    public IReadOnlyList<Angzarr.Cover> PrepareDestinations(Angzarr.EventBook? source)
    {
        if (source == null || source.Pages.Count == 0)
            return Array.Empty<Angzarr.Cover>();

        var eventPage = source.Pages[^1]; // Last page
        var eventAny = eventPage.Event;
        if (eventAny == null)
            return Array.Empty<Angzarr.Cover>();

        return _handler.Prepare(source, eventAny);
    }

    /// <summary>
    /// Dispatch an event to the saga handler.
    /// </summary>
    /// <param name="source">Source event book.</param>
    /// <param name="destinations">Fetched destination aggregate states.</param>
    /// <returns>Saga response with commands and events.</returns>
    public Angzarr.SagaResponse Dispatch(
        Angzarr.EventBook source,
        IReadOnlyList<Angzarr.EventBook>? destinations = null
    )
    {
        if (source.Pages.Count == 0)
            throw new InvalidArgumentError("Source event book has no events");

        var eventPage = source.Pages[^1]; // Last page
        var eventAny = eventPage.Event;
        if (eventAny == null)
            throw new InvalidArgumentError("Missing event payload");

        // Check for Notification
        if (eventAny.TypeUrl.EndsWith("Notification"))
            return DispatchNotification(eventAny);

        var handlerResponse = _handler.Execute(
            source,
            eventAny,
            destinations ?? Array.Empty<Angzarr.EventBook>()
        );

        var response = new Angzarr.SagaResponse();
        response.Commands.AddRange(handlerResponse.Commands);
        response.Events.AddRange(handlerResponse.Events);
        return response;
    }

    private Angzarr.SagaResponse DispatchNotification(Any eventAny)
    {
        var notification = eventAny.Unpack<Angzarr.Notification>();

        string targetDomain = "";
        string targetCommand = "";

        if (notification.Payload != null)
        {
            try
            {
                var rejection = notification.Payload.Unpack<Angzarr.RejectionNotification>();
                if (rejection.RejectedCommand?.Pages.Count > 0)
                {
                    targetDomain = rejection.RejectedCommand.Cover?.Domain ?? "";
                    targetCommand = Helpers.TypeNameFromUrl(
                        rejection.RejectedCommand.Pages[0].Command?.TypeUrl ?? ""
                    );
                }
            }
            catch (InvalidProtocolBufferException)
            {
                // Malformed rejection notification
            }
        }

        var rejectionResponse = _handler.OnRejected(notification, targetDomain, targetCommand);

        var response = new Angzarr.SagaResponse();
        if (rejectionResponse.Events != null)
            response.Events.Add(rejectionResponse.Events);
        return response;
    }
}
